//! PuTTY PPK v2/v3 private-key loader.
//!
//! russh/ssh-key natively understand OpenSSH, PKCS#8 and PEM keys, but not
//! PuTTY's PPK container. This module verifies the PPK MAC, decrypts encrypted
//! private blobs in memory, and converts the supported key material into the
//! same `ssh_key::PrivateKey` type used by the rest of the SSH stack.

use std::fs;
use std::path::Path;

use aes::Aes256;
use anyhow::{anyhow, bail, Context, Result};
use argon2::{Algorithm as ArgonAlgorithm, Argon2, Params, Version};
use base64::engine::general_purpose::STANDARD;
use base64::Engine as _;
use cbc::cipher::block_padding::NoPadding;
use cbc::cipher::{BlockDecryptMut, KeyIvInit};
use hmac::{Hmac, Mac};
use sha1::{Digest as _, Sha1};
use sha2::Sha256;
use ssh_encoding::Decode;
use ssh_key::private::{
    EcdsaKeypair, EcdsaPrivateKey, Ed25519Keypair, KeypairData, RsaKeypair, RsaPrivateKey,
};
use ssh_key::public::{EcdsaPublicKey, KeyData};
use ssh_key::{Mpint, PrivateKey, PublicKey};
use zeroize::Zeroizing;

type Aes256CbcDec = cbc::Decryptor<Aes256>;

#[derive(Debug)]
struct PpkFile {
    version: u8,
    algorithm: String,
    encryption: String,
    comment: String,
    public_blob: Vec<u8>,
    private_blob: Vec<u8>,
    private_mac: Vec<u8>,
    derivation: Option<ArgonSettings>,
}

#[derive(Debug)]
struct ArgonSettings {
    algorithm: ArgonAlgorithm,
    memory_kib: u32,
    passes: u32,
    parallelism: u32,
    salt: Vec<u8>,
}

/// Load a PuTTY PPK file, including encrypted v2/v3 keys.
pub(crate) fn load(path: &Path, passphrase: Option<&str>) -> Result<PrivateKey> {
    const MAX_PPK_SIZE: u64 = 16 * 1024 * 1024;
    if fs::metadata(path)
        .with_context(|| format!("failed to inspect PPK {}", path.display()))?
        .len()
        > MAX_PPK_SIZE
    {
        bail!("PPK file is unreasonably large");
    }
    let raw = fs::read_to_string(path)
        .with_context(|| format!("failed to read PPK {}", path.display()))?;
    load_str(&raw, passphrase.unwrap_or(""))
}

fn load_str(raw: &str, passphrase: &str) -> Result<PrivateKey> {
    let ppk = parse(raw)?;
    let (cipher_key, iv, mac_key) = derive_key_material(&ppk, passphrase)?;
    let private_plain = decrypt_private(&ppk, &cipher_key, &iv)?;
    verify_mac(&ppk, &private_plain, &mac_key)?;
    convert_key(&ppk, &private_plain)
}

fn parse(raw: &str) -> Result<PpkFile> {
    let lines: Vec<&str> = raw
        .lines()
        .map(|line| line.trim_end_matches('\r'))
        .collect();
    let mut cursor = 0usize;
    let first = next_line(&lines, &mut cursor, "PPK header")?;
    let (version, algorithm) = if let Some(value) = first.strip_prefix("PuTTY-User-Key-File-2: ") {
        (2, value.to_string())
    } else if let Some(value) = first.strip_prefix("PuTTY-User-Key-File-3: ") {
        (3, value.to_string())
    } else {
        bail!("not a supported PuTTY PPK v2/v3 file");
    };

    let encryption =
        header(next_line(&lines, &mut cursor, "Encryption")?, "Encryption")?.to_string();
    if encryption != "none" && encryption != "aes256-cbc" {
        bail!("unsupported PPK encryption: {encryption}");
    }
    let comment = header(next_line(&lines, &mut cursor, "Comment")?, "Comment")?.to_string();
    let public_blob = read_blob(&lines, &mut cursor, "Public-Lines")?;

    let derivation = if version == 3 && encryption != "none" {
        let algorithm = match header(
            next_line(&lines, &mut cursor, "Key-Derivation")?,
            "Key-Derivation",
        )? {
            "Argon2d" => ArgonAlgorithm::Argon2d,
            "Argon2i" => ArgonAlgorithm::Argon2i,
            "Argon2id" => ArgonAlgorithm::Argon2id,
            other => bail!("unsupported PPK key derivation: {other}"),
        };
        let memory_kib = parse_u32_header(&lines, &mut cursor, "Argon2-Memory")?;
        let passes = parse_u32_header(&lines, &mut cursor, "Argon2-Passes")?;
        let parallelism = parse_u32_header(&lines, &mut cursor, "Argon2-Parallelism")?;
        let salt = hex::decode(header(
            next_line(&lines, &mut cursor, "Argon2-Salt")?,
            "Argon2-Salt",
        )?)
        .context("invalid PPK Argon2 salt")?;
        Some(ArgonSettings {
            algorithm,
            memory_kib,
            passes,
            parallelism,
            salt,
        })
    } else {
        None
    };

    let private_blob = read_blob(&lines, &mut cursor, "Private-Lines")?;
    let private_mac = hex::decode(header(
        next_line(&lines, &mut cursor, "Private-MAC")?,
        "Private-MAC",
    )?)
    .context("invalid PPK MAC")?;

    Ok(PpkFile {
        version,
        algorithm,
        encryption,
        comment,
        public_blob,
        private_blob,
        private_mac,
        derivation,
    })
}

fn next_line<'a>(lines: &'a [&str], cursor: &mut usize, what: &str) -> Result<&'a str> {
    let line = lines
        .get(*cursor)
        .copied()
        .ok_or_else(|| anyhow!("missing PPK {what}"))?;
    *cursor += 1;
    Ok(line)
}

fn header<'a>(line: &'a str, name: &str) -> Result<&'a str> {
    line.strip_prefix(name)
        .and_then(|rest| rest.strip_prefix(": "))
        .ok_or_else(|| anyhow!("expected PPK {name} header"))
}

fn parse_u32_header(lines: &[&str], cursor: &mut usize, name: &str) -> Result<u32> {
    header(next_line(lines, cursor, name)?, name)?
        .parse()
        .with_context(|| format!("invalid PPK {name}"))
}

fn read_blob(lines: &[&str], cursor: &mut usize, name: &str) -> Result<Vec<u8>> {
    let count: usize = header(next_line(lines, cursor, name)?, name)?
        .parse()
        .with_context(|| format!("invalid PPK {name} count"))?;
    let end = cursor
        .checked_add(count)
        .filter(|end| *end <= lines.len())
        .ok_or_else(|| anyhow!("truncated PPK {name} data"))?;
    let joined = lines[*cursor..end].concat();
    *cursor = end;
    STANDARD
        .decode(joined)
        .with_context(|| format!("invalid base64 in PPK {name}"))
}

fn derive_key_material(ppk: &PpkFile, passphrase: &str) -> Result<(Vec<u8>, Vec<u8>, Vec<u8>)> {
    if ppk.version == 3 {
        if ppk.encryption == "none" {
            return Ok((Vec::new(), Vec::new(), Vec::new()));
        }
        let settings = ppk
            .derivation
            .as_ref()
            .ok_or_else(|| anyhow!("missing PPK Argon2 settings"))?;
        if settings.memory_kib > 512 * 1024 || settings.passes > 1_000 || settings.parallelism > 64
        {
            bail!("PPK Argon2 parameters exceed safe local limits");
        }
        let params = Params::new(
            settings.memory_kib,
            settings.passes,
            settings.parallelism,
            Some(80),
        )
        .map_err(|error| anyhow!("invalid PPK Argon2 parameters: {error}"))?;
        let argon = Argon2::new(settings.algorithm, Version::V0x13, params);
        let mut output = vec![0u8; 80];
        argon
            .hash_password_into(passphrase.as_bytes(), &settings.salt, &mut output)
            .map_err(|error| anyhow!("PPK Argon2 derivation failed: {error}"))?;
        return Ok((
            output[..32].to_vec(),
            output[32..48].to_vec(),
            output[48..].to_vec(),
        ));
    }

    let effective_passphrase = if ppk.encryption == "none" {
        ""
    } else {
        passphrase
    };
    let mut mac_hasher = Sha1::new();
    mac_hasher.update(b"putty-private-key-file-mac-key");
    mac_hasher.update(effective_passphrase.as_bytes());
    let mac_key = mac_hasher.finalize().to_vec();
    if ppk.encryption == "none" {
        return Ok((Vec::new(), Vec::new(), mac_key));
    }

    let mut cipher_key = Vec::with_capacity(40);
    for sequence in [0u32, 1] {
        let mut hasher = Sha1::new();
        hasher.update(sequence.to_be_bytes());
        hasher.update(effective_passphrase.as_bytes());
        cipher_key.extend_from_slice(&hasher.finalize());
    }
    cipher_key.truncate(32);
    Ok((cipher_key, vec![0u8; 16], mac_key))
}

fn decrypt_private(ppk: &PpkFile, key: &[u8], iv: &[u8]) -> Result<Zeroizing<Vec<u8>>> {
    if ppk.encryption == "none" {
        return Ok(Zeroizing::new(ppk.private_blob.clone()));
    }
    if ppk.private_blob.len() % 16 != 0 {
        bail!("encrypted PPK private blob is not AES block aligned");
    }
    let plain = Aes256CbcDec::new_from_slices(key, iv)
        .context("invalid PPK AES key/IV")?
        .decrypt_padded_vec_mut::<NoPadding>(&ppk.private_blob)
        .map_err(|_| anyhow!("failed to decrypt PPK private blob"))?;
    Ok(Zeroizing::new(plain))
}

fn verify_mac(ppk: &PpkFile, private_plain: &[u8], mac_key: &[u8]) -> Result<()> {
    let mut preimage = Vec::new();
    put_string(&mut preimage, ppk.algorithm.as_bytes())?;
    put_string(&mut preimage, ppk.encryption.as_bytes())?;
    put_string(&mut preimage, ppk.comment.as_bytes())?;
    put_string(&mut preimage, &ppk.public_blob)?;
    put_string(&mut preimage, private_plain)?;

    if ppk.version == 3 {
        let mut mac = Hmac::<Sha256>::new_from_slice(mac_key).context("invalid PPK MAC key")?;
        mac.update(&preimage);
        mac.verify_slice(&ppk.private_mac)
            .map_err(|_| anyhow!("PPK MAC mismatch (wrong passphrase or damaged key)"))
    } else {
        let mut mac = Hmac::<Sha1>::new_from_slice(mac_key).context("invalid PPK MAC key")?;
        mac.update(&preimage);
        mac.verify_slice(&ppk.private_mac)
            .map_err(|_| anyhow!("PPK MAC mismatch (wrong passphrase or damaged key)"))
    }
}

fn put_string(out: &mut Vec<u8>, value: &[u8]) -> Result<()> {
    let len = u32::try_from(value.len()).context("PPK value is too large")?;
    out.extend_from_slice(&len.to_be_bytes());
    out.extend_from_slice(value);
    Ok(())
}

fn read_string<'a>(input: &mut &'a [u8]) -> Result<&'a [u8]> {
    let len_bytes: [u8; 4] = input
        .get(..4)
        .ok_or_else(|| anyhow!("truncated PPK private value"))?
        .try_into()
        .expect("length checked");
    let len = u32::from_be_bytes(len_bytes) as usize;
    let value = input
        .get(4..4 + len)
        .ok_or_else(|| anyhow!("truncated PPK private value"))?;
    *input = &input[4 + len..];
    Ok(value)
}

fn positive_bytes(value: &[u8]) -> &[u8] {
    value.strip_prefix(&[0]).unwrap_or(value)
}

fn fixed_scalar<const N: usize>(value: &[u8]) -> Result<[u8; N]> {
    let value = positive_bytes(value);
    if value.len() > N {
        bail!("PPK private scalar is too large");
    }
    let mut out = [0u8; N];
    out[N - value.len()..].copy_from_slice(value);
    Ok(out)
}

fn decode_ecdsa_private<const N: usize>(value: &[u8]) -> Result<EcdsaPrivateKey<N>> {
    let scalar = fixed_scalar::<N>(value)?;
    let mut encoded = Vec::with_capacity(N + 5);
    if scalar[0] >= 0x80 {
        encoded.extend_from_slice(&u32::try_from(N + 1)?.to_be_bytes());
        encoded.push(0);
    } else {
        encoded.extend_from_slice(&u32::try_from(N)?.to_be_bytes());
    }
    encoded.extend_from_slice(&scalar);
    let mut reader = encoded.as_slice();
    EcdsaPrivateKey::<N>::decode(&mut reader).context("invalid PPK ECDSA private scalar")
}

fn convert_key(ppk: &PpkFile, private_plain: &[u8]) -> Result<PrivateKey> {
    let public = PublicKey::from_bytes(&ppk.public_blob).context("invalid PPK public key")?;
    if public.algorithm().as_str() != ppk.algorithm {
        bail!("PPK algorithm header does not match its public key");
    }
    let mut private = private_plain;

    let keypair = match public.key_data() {
        KeyData::Rsa(public) => {
            let d = Mpint::from_bytes(read_string(&mut private)?).context("invalid PPK RSA d")?;
            let p = Mpint::from_bytes(read_string(&mut private)?).context("invalid PPK RSA p")?;
            let q = Mpint::from_bytes(read_string(&mut private)?).context("invalid PPK RSA q")?;
            let iqmp =
                Mpint::from_bytes(read_string(&mut private)?).context("invalid PPK RSA iqmp")?;
            KeypairData::Rsa(RsaKeypair {
                public: public.clone(),
                private: RsaPrivateKey { d, iqmp, p, q },
            })
        }
        KeyData::Ed25519(public) => {
            let seed = fixed_scalar::<32>(read_string(&mut private)?)?;
            let keypair = Ed25519Keypair::from_seed(&seed);
            if &keypair.public != public {
                bail!("PPK Ed25519 public/private key mismatch");
            }
            KeypairData::Ed25519(keypair)
        }
        KeyData::Ecdsa(public) => {
            let scalar = read_string(&mut private)?;
            let keypair = match public {
                EcdsaPublicKey::NistP256(public) => EcdsaKeypair::NistP256 {
                    public: *public,
                    private: decode_ecdsa_private::<32>(scalar)?,
                },
                EcdsaPublicKey::NistP384(public) => EcdsaKeypair::NistP384 {
                    public: *public,
                    private: decode_ecdsa_private::<48>(scalar)?,
                },
                EcdsaPublicKey::NistP521(public) => EcdsaKeypair::NistP521 {
                    public: *public,
                    private: decode_ecdsa_private::<66>(scalar)?,
                },
            };
            KeypairData::Ecdsa(keypair)
        }
        _ => bail!("unsupported PPK key algorithm: {}", ppk.algorithm),
    };

    PrivateKey::new(keypair, ppk.comment.clone()).context("invalid PPK private key")
}

#[cfg(test)]
mod tests {
    use super::*;
    use cbc::cipher::BlockEncryptMut;

    fn encrypted_ed25519_fixture(version: u8, passphrase: &str) -> String {
        let seed = [9u8; 32];
        let pair = Ed25519Keypair::from_seed(&seed);
        let key = PrivateKey::new(KeypairData::Ed25519(pair), "encrypted fixture").unwrap();
        let public = key.public_key().to_bytes().unwrap();
        let mut private = Vec::new();
        put_string(&mut private, &seed).unwrap();
        private.resize(private.len().next_multiple_of(16), 0x5a);

        let derivation = (version == 3).then(|| ArgonSettings {
            algorithm: ArgonAlgorithm::Argon2id,
            memory_kib: 32,
            passes: 2,
            parallelism: 1,
            salt: vec![1, 2, 3, 4, 5, 6, 7, 8],
        });
        let mut ppk = PpkFile {
            version,
            algorithm: "ssh-ed25519".into(),
            encryption: "aes256-cbc".into(),
            comment: "encrypted fixture".into(),
            public_blob: public.clone(),
            private_blob: Vec::new(),
            private_mac: Vec::new(),
            derivation,
        };
        let (key, iv, mac_key) = derive_key_material(&ppk, passphrase).unwrap();
        let mut preimage = Vec::new();
        put_string(&mut preimage, ppk.algorithm.as_bytes()).unwrap();
        put_string(&mut preimage, ppk.encryption.as_bytes()).unwrap();
        put_string(&mut preimage, ppk.comment.as_bytes()).unwrap();
        put_string(&mut preimage, &public).unwrap();
        put_string(&mut preimage, &private).unwrap();
        let mac = if version == 3 {
            let mut mac = Hmac::<Sha256>::new_from_slice(&mac_key).unwrap();
            mac.update(&preimage);
            mac.finalize().into_bytes().to_vec()
        } else {
            let mut mac = Hmac::<Sha1>::new_from_slice(&mac_key).unwrap();
            mac.update(&preimage);
            mac.finalize().into_bytes().to_vec()
        };
        ppk.private_blob = cbc::Encryptor::<Aes256>::new_from_slices(&key, &iv)
            .unwrap()
            .encrypt_padded_vec_mut::<NoPadding>(&private);

        let derivation_headers = if version == 3 {
            "Key-Derivation: Argon2id\n\
             Argon2-Memory: 32\n\
             Argon2-Passes: 2\n\
             Argon2-Parallelism: 1\n\
             Argon2-Salt: 0102030405060708\n"
        } else {
            ""
        };
        format!(
            "PuTTY-User-Key-File-{version}: ssh-ed25519\n\
             Encryption: aes256-cbc\n\
             Comment: encrypted fixture\n\
             Public-Lines: 1\n{}\n{}\
             Private-Lines: 1\n{}\n\
             Private-MAC: {}\n",
            STANDARD.encode(public),
            derivation_headers,
            STANDARD.encode(&ppk.private_blob),
            hex::encode(mac)
        )
    }

    #[test]
    fn loads_unencrypted_v3_ed25519() {
        let seed = [7u8; 32];
        let pair = Ed25519Keypair::from_seed(&seed);
        let key = PrivateKey::new(KeypairData::Ed25519(pair), "fixture").unwrap();
        let public = key.public_key().to_bytes().unwrap();
        let mut private = Vec::new();
        put_string(&mut private, &seed).unwrap();

        let mut preimage = Vec::new();
        put_string(&mut preimage, b"ssh-ed25519").unwrap();
        put_string(&mut preimage, b"none").unwrap();
        put_string(&mut preimage, b"fixture").unwrap();
        put_string(&mut preimage, &public).unwrap();
        put_string(&mut preimage, &private).unwrap();
        let mut mac = Hmac::<Sha256>::new_from_slice(&[]).unwrap();
        mac.update(&preimage);
        let mac = hex::encode(mac.finalize().into_bytes());

        let text = format!(
            "PuTTY-User-Key-File-3: ssh-ed25519\n\
             Encryption: none\n\
             Comment: fixture\n\
             Public-Lines: 1\n{}\n\
             Private-Lines: 1\n{}\n\
             Private-MAC: {}\n",
            STANDARD.encode(public),
            STANDARD.encode(private),
            mac
        );
        let loaded = load_str(&text, "").unwrap();
        assert_eq!(loaded.public_key(), key.public_key());
    }

    #[test]
    fn rejects_a_bad_mac() {
        let text = "PuTTY-User-Key-File-3: ssh-ed25519\n\
                    Encryption: none\n\
                    Comment: bad\n\
                    Public-Lines: 1\nAAAA\n\
                    Private-Lines: 1\nAAAA\n\
                    Private-MAC: 00\n";
        assert!(load_str(text, "").is_err());
    }

    #[test]
    fn loads_encrypted_v2_and_v3() {
        for version in [2, 3] {
            let fixture = encrypted_ed25519_fixture(version, "correct horse");
            assert!(load_str(&fixture, "correct horse").is_ok());
            let error = load_str(&fixture, "wrong").unwrap_err().to_string();
            assert!(error.contains("MAC mismatch"));
        }
    }
}
