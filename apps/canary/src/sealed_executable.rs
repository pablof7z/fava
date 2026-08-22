//! Linux sealed-memory executable image shared by the controlled launchers.

use std::fs::File;
use std::io::{self, Write};
use std::os::unix::fs::{FileExt, MetadataExt};

use rustix::fs::{
    MemfdFlags, Mode, SealFlags, fchmod, fcntl_add_seals, fcntl_get_seals, memfd_create,
};
use sha2::{Digest, Sha256};

const REQUIRED_SEALS: SealFlags = SealFlags::SHRINK
    .union(SealFlags::GROW)
    .union(SealFlags::WRITE)
    .union(SealFlags::SEAL);

/// One exact immutable executable object, sealed before its digest is computed.
#[derive(Debug)]
#[allow(
    dead_code,
    reason = "the shared source is compiled once for the library and once for the launcher binary"
)]
pub(crate) struct SealedExecutable {
    file: File,
    device: u64,
    inode: u64,
    sha256: String,
}

impl SealedExecutable {
    pub(crate) fn copy_from(source: &File, maximum: u64) -> io::Result<Self> {
        let source_before = source.metadata()?;
        if !source_before.is_file() || source_before.len() == 0 || source_before.len() > maximum {
            return Err(io::Error::new(
                io::ErrorKind::InvalidData,
                "executable source was not a bounded regular file",
            ));
        }
        let owned = memfd_create(
            "fava-sealed-executable",
            MemfdFlags::ALLOW_SEALING | MemfdFlags::CLOEXEC,
        )?;
        let mut file = File::from(owned);
        let mut offset = 0_u64;
        let mut buffer = [0_u8; 16_384];
        while offset < source_before.len() {
            let wanted = usize::try_from((source_before.len() - offset).min(buffer.len() as u64))
                .map_err(io::Error::other)?;
            let read = source.read_at(&mut buffer[..wanted], offset)?;
            if read == 0 {
                return Err(io::Error::new(
                    io::ErrorKind::UnexpectedEof,
                    "executable source changed during sealed copy",
                ));
            }
            file.write_all(&buffer[..read])?;
            offset = offset
                .checked_add(u64::try_from(read).map_err(io::Error::other)?)
                .ok_or_else(|| io::Error::other("executable byte count overflow"))?;
        }
        let source_after = source.metadata()?;
        if source_before.dev() != source_after.dev()
            || source_before.ino() != source_after.ino()
            || source_before.len() != source_after.len()
        {
            return Err(io::Error::other(
                "executable source changed during sealed copy",
            ));
        }
        file.sync_all()?;
        fchmod(&file, Mode::RUSR | Mode::XUSR)?;
        fcntl_add_seals(&file, REQUIRED_SEALS)?;
        if !fcntl_get_seals(&file)?.contains(REQUIRED_SEALS) {
            return Err(io::Error::other(
                "executable memory object was not fully sealed",
            ));
        }
        let metadata = file.metadata()?;
        let sha256 = descriptor_sha256(&file, metadata.len())?;
        Ok(Self {
            file,
            device: metadata.dev(),
            inode: metadata.ino(),
            sha256,
        })
    }

    pub(crate) fn try_clone(&self) -> io::Result<File> {
        self.file.try_clone()
    }

    pub(crate) fn sha256(&self) -> &str {
        &self.sha256
    }

    #[cfg(test)]
    pub(crate) fn bytes(&self) -> u64 {
        self.file.metadata().map(|item| item.len()).unwrap_or(0)
    }

    #[allow(
        dead_code,
        reason = "the launcher binary does not publish the library supervisor's inode fact"
    )]
    pub(crate) fn device(&self) -> u64 {
        self.device
    }

    #[allow(
        dead_code,
        reason = "the launcher binary does not publish the library supervisor's inode fact"
    )]
    pub(crate) fn inode(&self) -> u64 {
        self.inode
    }

    #[cfg(test)]
    pub(crate) fn try_overwrite(&self, bytes: &[u8]) -> io::Result<usize> {
        self.file.write_at(bytes, 0)
    }
}

fn descriptor_sha256(file: &File, bytes: u64) -> io::Result<String> {
    let mut digest = Sha256::new();
    let mut offset = 0_u64;
    let mut buffer = [0_u8; 16_384];
    while offset < bytes {
        let wanted =
            usize::try_from((bytes - offset).min(buffer.len() as u64)).map_err(io::Error::other)?;
        let read = file.read_at(&mut buffer[..wanted], offset)?;
        if read == 0 {
            return Err(io::Error::new(
                io::ErrorKind::UnexpectedEof,
                "sealed executable ended before its bound",
            ));
        }
        digest.update(&buffer[..read]);
        offset = offset
            .checked_add(u64::try_from(read).map_err(io::Error::other)?)
            .ok_or_else(|| io::Error::other("sealed executable byte count overflow"))?;
    }
    Ok(hex::encode(digest.finalize()))
}

#[cfg(test)]
mod tests {
    use std::fs;
    use std::os::unix::fs::PermissionsExt;

    use sha2::Digest;
    use tempfile::TempDir;

    use super::SealedExecutable;

    #[test]
    fn same_length_in_place_mutation_is_refused_after_sealing() {
        let fixture = TempDir::new().expect("fixture");
        let source_path = fixture.path().join("candidate");
        fs::write(&source_path, b"reviewed executable bytes").expect("source");
        fs::set_permissions(&source_path, fs::Permissions::from_mode(0o500)).expect("mode");
        let source = fs::File::open(source_path).expect("open source");
        let sealed = SealedExecutable::copy_from(&source, 1024).expect("sealed copy");
        let replacement = vec![b'x'; usize::try_from(sealed.bytes()).unwrap()];
        assert!(
            sealed.try_overwrite(&replacement).is_err(),
            "same-size in-place mutation crossed the executable seal"
        );
        assert_eq!(
            sealed.sha256(),
            hex::encode(sha2::Sha256::digest(b"reviewed executable bytes"))
        );
    }
}
