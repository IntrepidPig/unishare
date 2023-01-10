//! This module implements an abstraction over a low-level fuse filesystem.

use thiserror::Error;
use tracing::{info, warn, error, debug, trace};

use std::{marker::PhantomData, time::{Duration, SystemTime}, os::unix::prelude::OsStrExt};
use unishare_common::storage::*;

pub trait Filesystem {
	fn getattr(&mut self, ino: u64) -> FuseResult<Attr>;
	fn lookup(&mut self, parent: u64, name: &str) -> FuseResult<Attr>;
	fn open(&mut self, ino: u64, flags: i32) -> FuseResult<Opened>;
	fn read(&mut self, ino: u64, fh: u64, offset: i64, size: u32) -> FuseResult<Vec<u8>>;
}

pub struct FilesystemImpl<T>(pub T);

impl<T> fuser::Filesystem for FilesystemImpl<T> where T: Filesystem {
	fn init(&mut self, _req: &fuser::Request<'_>, _config: &mut fuser::KernelConfig) -> Result<(), libc::c_int> {
		debug!("[Default Impl] init()");
		Ok(())
	}

	fn destroy(&mut self) {
		debug!("[Default Impl] destroy()");
	}

	fn lookup(&mut self, _req: &fuser::Request<'_>, parent: u64, name: &std::ffi::OsStr, reply: fuser::ReplyEntry) {
		debug!(parent, ?name, "LOOKUP");
		match self.0.lookup(parent, std::str::from_utf8(name.as_bytes()).unwrap()) {
			Ok(t) => reply.entry(&Duration::from_secs(1), &t.into(), 1),
			Err(e) => reply.error(e.into()),
		}
	}

	fn forget(&mut self, _req: &fuser::Request<'_>, ino: u64, nlookup: u64) {
		debug!(
			"[Not Implemented] forget(ino: {:#x?}, nlookup: {})",
			ino, nlookup
		);
	}

	fn getattr(&mut self, _req: &fuser::Request<'_>, ino: u64, reply: fuser::ReplyAttr) {
		debug!(ino, "GETATTR");
		match self.0.getattr(ino) {
			Ok(t) => reply.attr(&Duration::from_secs(1), &t.into()),
			Err(e) => reply.error(e.into()),
		}
	}

	fn setattr(
		&mut self,
		_req: &fuser::Request<'_>,
		ino: u64,
		mode: Option<u32>,
		uid: Option<u32>,
		gid: Option<u32>,
		size: Option<u64>,
		_atime: Option<fuser::TimeOrNow>,
		_mtime: Option<fuser::TimeOrNow>,
		_ctime: Option<std::time::SystemTime>,
		fh: Option<u64>,
		_crtime: Option<std::time::SystemTime>,
		_chgtime: Option<std::time::SystemTime>,
		_bkuptime: Option<std::time::SystemTime>,
		flags: Option<u32>,
		reply: fuser::ReplyAttr,
	) {
		debug!(
			"[Not Implemented] setattr(ino: {:#x?}, mode: {:?}, uid: {:?}, \
			gid: {:?}, size: {:?}, fh: {:?}, flags: {:?})",
			ino, mode, uid, gid, size, fh, flags
		);
		reply.error(libc::ENOSYS);
	}

	fn readlink(&mut self, _req: &fuser::Request<'_>, ino: u64, reply: fuser::ReplyData) {
		debug!("[Not Implemented] readlink(ino: {:#x?})", ino);
		reply.error(libc::ENOSYS);
	}

	fn mknod(
		&mut self,
		_req: &fuser::Request<'_>,
		parent: u64,
		name: &std::ffi::OsStr,
		mode: u32,
		umask: u32,
		rdev: u32,
		reply: fuser::ReplyEntry,
	) {
		debug!(
			"[Not Implemented] mknod(parent: {:#x?}, name: {:?}, mode: {}, \
			umask: {:#x?}, rdev: {})",
			parent, name, mode, umask, rdev
		);
		reply.error(libc::ENOSYS);
	}

	fn mkdir(
		&mut self,
		_req: &fuser::Request<'_>,
		parent: u64,
		name: &std::ffi::OsStr,
		mode: u32,
		umask: u32,
		reply: fuser::ReplyEntry,
	) {
		debug!(
			"[Not Implemented] mkdir(parent: {:#x?}, name: {:?}, mode: {}, umask: {:#x?})",
			parent, name, mode, umask
		);
		reply.error(libc::ENOSYS);
	}

	fn unlink(&mut self, _req: &fuser::Request<'_>, parent: u64, name: &std::ffi::OsStr, reply: fuser::ReplyEmpty) {
		debug!(
			"[Not Implemented] unlink(parent: {:#x?}, name: {:?})",
			parent, name,
		);
		reply.error(libc::ENOSYS);
	}

	fn rmdir(&mut self, _req: &fuser::Request<'_>, parent: u64, name: &std::ffi::OsStr, reply: fuser::ReplyEmpty) {
		debug!(
			"[Not Implemented] rmdir(parent: {:#x?}, name: {:?})",
			parent, name,
		);
		reply.error(libc::ENOSYS);
	}

	fn symlink(
		&mut self,
		_req: &fuser::Request<'_>,
		parent: u64,
		name: &std::ffi::OsStr,
		link: &std::path::Path,
		reply: fuser::ReplyEntry,
	) {
		debug!(
			"[Not Implemented] symlink(parent: {:#x?}, name: {:?}, link: {:?})",
			parent, name, link,
		);
		reply.error(libc::EPERM);
	}

	fn rename(
		&mut self,
		_req: &fuser::Request<'_>,
		parent: u64,
		name: &std::ffi::OsStr,
		newparent: u64,
		newname: &std::ffi::OsStr,
		flags: u32,
		reply: fuser::ReplyEmpty,
	) {
		debug!(
			"[Not Implemented] rename(parent: {:#x?}, name: {:?}, newparent: {:#x?}, \
			newname: {:?}, flags: {})",
			parent, name, newparent, newname, flags,
		);
		reply.error(libc::ENOSYS);
	}

	fn link(
		&mut self,
		_req: &fuser::Request<'_>,
		ino: u64,
		newparent: u64,
		newname: &std::ffi::OsStr,
		reply: fuser::ReplyEntry,
	) {
		debug!(
			"[Not Implemented] link(ino: {:#x?}, newparent: {:#x?}, newname: {:?})",
			ino, newparent, newname
		);
		reply.error(libc::EPERM);
	}

	fn open(&mut self, _req: &fuser::Request<'_>, ino: u64, flags: i32, reply: fuser::ReplyOpen) {
		debug!(ino, flags, "OPEN");
		match self.0.open(ino, flags) {
			Ok(t) => reply.opened(t.fh, t.flags),
			Err(e) => reply.error(e.into()),
		}
	}

	fn read(
		&mut self,
		_req: &fuser::Request<'_>,
		ino: u64,
		fh: u64,
		offset: i64,
		size: u32,
		flags: i32,
		lock_owner: Option<u64>,
		reply: fuser::ReplyData,
	) {
		debug!(ino, fh, offset, size, flags, lock_owner, "READ");
		match self.0.read(ino, fh, offset, size) {
			Ok(t) => reply.data(&t),
			Err(e) => reply.error(e.into()),
		}
	}

	fn write(
		&mut self,
		_req: &fuser::Request<'_>,
		ino: u64,
		fh: u64,
		offset: i64,
		data: &[u8],
		write_flags: u32,
		flags: i32,
		lock_owner: Option<u64>,
		reply: fuser::ReplyWrite,
	) {
		debug!(
			"[Not Implemented] write(ino: {:#x?}, fh: {}, offset: {}, data.len(): {}, \
			write_flags: {:#x?}, flags: {:#x?}, lock_owner: {:?})",
			ino,
			fh,
			offset,
			data.len(),
			write_flags,
			flags,
			lock_owner
		);
		reply.error(libc::ENOSYS);
	}

	fn flush(&mut self, _req: &fuser::Request<'_>, ino: u64, fh: u64, lock_owner: u64, reply: fuser::ReplyEmpty) {
		debug!(
			"[Not Implemented] flush(ino: {:#x?}, fh: {}, lock_owner: {:?})",
			ino, fh, lock_owner
		);
		reply.error(libc::ENOSYS);
	}

	fn release(
		&mut self,
		_req: &fuser::Request<'_>,
		_ino: u64,
		_fh: u64,
		_flags: i32,
		_lock_owner: Option<u64>,
		_flush: bool,
		reply: fuser::ReplyEmpty,
	) {
		reply.ok();
	}

	fn fsync(&mut self, _req: &fuser::Request<'_>, ino: u64, fh: u64, datasync: bool, reply: fuser::ReplyEmpty) {
		debug!(
			"[Not Implemented] fsync(ino: {:#x?}, fh: {}, datasync: {})",
			ino, fh, datasync
		);
		reply.error(libc::ENOSYS);
	}

	fn opendir(&mut self, _req: &fuser::Request<'_>, ino: u64, flags: i32, reply: fuser::ReplyOpen) {
		debug!(
			"[Default Impl] opendir(ino: {:#x?}, flags: {})",
			ino, flags
		);
		reply.opened(0, 0);
	}

	fn readdir(
		&mut self,
		_req: &fuser::Request<'_>,
		ino: u64,
		fh: u64,
		offset: i64,
		reply: fuser::ReplyDirectory,
	) {
		warn!(
			"[Not Implemented] readdir(ino: {:#x?}, fh: {}, offset: {})",
			ino, fh, offset
		);
		reply.error(libc::ENOSYS);
	}

	fn readdirplus(
		&mut self,
		_req: &fuser::Request<'_>,
		ino: u64,
		fh: u64,
		offset: i64,
		reply: fuser::ReplyDirectoryPlus,
	) {
		debug!(
			"[Not Implemented] readdirplus(ino: {:#x?}, fh: {}, offset: {})",
			ino, fh, offset
		);
		reply.error(libc::ENOSYS);
	}

	fn releasedir(
		&mut self,
		_req: &fuser::Request<'_>,
		ino: u64,
		fh: u64,
		flags: i32,
		reply: fuser::ReplyEmpty,
	) {
		debug!(
			"[Default Impl] releasedir(ino: {:#x?}, fh: {}, flags: {})",
			ino, fh, flags
		);
		reply.ok();
	}

	fn fsyncdir(
		&mut self,
		_req: &fuser::Request<'_>,
		ino: u64,
		fh: u64,
		datasync: bool,
		reply: fuser::ReplyEmpty,
	) {
		debug!(
			"[Not Implemented] fsyncdir(ino: {:#x?}, fh: {}, datasync: {})",
			ino, fh, datasync
		);
		reply.error(libc::ENOSYS);
	}

	fn statfs(&mut self, _req: &fuser::Request<'_>, ino: u64, reply: fuser::ReplyStatfs) {
		debug!(
			"[Not Implemented] statfs(ino: {:#x?})",
			ino,
		);
		//reply.statfs(0, 0, 0, 0, 0, 512, 255, 0);
		reply.error(libc::ENOSYS);
	}

	fn setxattr(
		&mut self,
		_req: &fuser::Request<'_>,
		ino: u64,
		name: &std::ffi::OsStr,
		_value: &[u8],
		flags: i32,
		position: u32,
		reply: fuser::ReplyEmpty,
	) {
		debug!(
			"[Not Implemented] setxattr(ino: {:#x?}, name: {:?}, flags: {:#x?}, position: {})",
			ino, name, flags, position
		);
		reply.error(libc::ENOSYS);
	}

	fn getxattr(
		&mut self,
		_req: &fuser::Request<'_>,
		ino: u64,
		name: &std::ffi::OsStr,
		size: u32,
		reply: fuser::ReplyXattr,
	) {
		debug!(
			"[Not Implemented] getxattr(ino: {:#x?}, name: {:?}, size: {})",
			ino, name, size
		);
		reply.error(libc::ENOSYS);
	}

	fn listxattr(&mut self, _req: &fuser::Request<'_>, ino: u64, size: u32, reply: fuser::ReplyXattr) {
		debug!(
			"[Not Implemented] listxattr(ino: {:#x?}, size: {})",
			ino, size
		);
		reply.error(libc::ENOSYS);
	}

	fn removexattr(&mut self, _req: &fuser::Request<'_>, ino: u64, name: &std::ffi::OsStr, reply: fuser::ReplyEmpty) {
		debug!(
			"[Not Implemented] removexattr(ino: {:#x?}, name: {:?})",
			ino, name
		);
		reply.error(libc::ENOSYS);
	}
	
	/// This is the same as the access(2) system call. It returns -ENOENT if the path doesn't exist,
	/// -EACCESS if the requested permission isn't available, or 0 for success. Note that it can be
	/// called on files, directories, or any other object that appears in the filesystem. This call
	/// is not required but is highly recommended. 
	fn access(&mut self, _req: &fuser::Request<'_>, ino: u64, mask: i32, reply: fuser::ReplyEmpty) {
		debug!("[Default Impl] access(ino: {:#x?}, mask: {})", ino, mask);
		//debug!("[Not Implemented] access(ino: {:#x?}, mask: {})", ino, mask);
		//reply.error(libc::ENOSYS);
		// TODO: actually check access
		reply.ok();
	}

	fn create(
		&mut self,
		_req: &fuser::Request<'_>,
		parent: u64,
		name: &std::ffi::OsStr,
		mode: u32,
		umask: u32,
		flags: i32,
		reply: fuser::ReplyCreate,
	) {
		debug!(
			"[Not Implemented] create(parent: {:#x?}, name: {:?}, mode: {}, umask: {:#x?}, \
			flags: {:#x?})",
			parent, name, mode, umask, flags
		);
		reply.error(libc::ENOSYS);
	}

	fn getlk(
		&mut self,
		_req: &fuser::Request<'_>,
		ino: u64,
		fh: u64,
		lock_owner: u64,
		start: u64,
		end: u64,
		typ: i32,
		pid: u32,
		reply: fuser::ReplyLock,
	) {
		debug!(
			"[Not Implemented] getlk(ino: {:#x?}, fh: {}, lock_owner: {}, start: {}, \
			end: {}, typ: {}, pid: {})",
			ino, fh, lock_owner, start, end, typ, pid
		);
		reply.error(libc::ENOSYS);
	}

	fn setlk(
		&mut self,
		_req: &fuser::Request<'_>,
		ino: u64,
		fh: u64,
		lock_owner: u64,
		start: u64,
		end: u64,
		typ: i32,
		pid: u32,
		sleep: bool,
		reply: fuser::ReplyEmpty,
	) {
		debug!(
			"[Not Implemented] setlk(ino: {:#x?}, fh: {}, lock_owner: {}, start: {}, \
			end: {}, typ: {}, pid: {}, sleep: {})",
			ino, fh, lock_owner, start, end, typ, pid, sleep
		);
		reply.error(libc::ENOSYS);
	}

	fn bmap(&mut self, _req: &fuser::Request<'_>, ino: u64, blocksize: u32, idx: u64, reply: fuser::ReplyBmap) {
		debug!(
			"[Not Implemented] bmap(ino: {:#x?}, blocksize: {}, idx: {})",
			ino, blocksize, idx,
		);
		reply.error(libc::ENOSYS);
	}

	fn ioctl(
		&mut self,
		_req: &fuser::Request<'_>,
		ino: u64,
		fh: u64,
		flags: u32,
		cmd: u32,
		in_data: &[u8],
		out_size: u32,
		reply: fuser::ReplyIoctl,
	) {
		debug!(
			"[Not Implemented] ioctl(ino: {:#x?}, fh: {}, flags: {}, cmd: {}, \
			in_data.len(): {}, out_size: {})",
			ino,
			fh,
			flags,
			cmd,
			in_data.len(),
			out_size,
		);
		reply.error(libc::ENOSYS);
	}

	fn fallocate(
		&mut self,
		_req: &fuser::Request<'_>,
		ino: u64,
		fh: u64,
		offset: i64,
		length: i64,
		mode: i32,
		reply: fuser::ReplyEmpty,
	) {
		debug!(
			"[Not Implemented] fallocate(ino: {:#x?}, fh: {}, offset: {}, \
			length: {}, mode: {})",
			ino, fh, offset, length, mode
		);
		reply.error(libc::ENOSYS);
	}

	fn lseek(
		&mut self,
		_req: &fuser::Request<'_>,
		ino: u64,
		fh: u64,
		offset: i64,
		whence: i32,
		reply: fuser::ReplyLseek,
	) {
		debug!(
			"[Not Implemented] lseek(ino: {:#x?}, fh: {}, offset: {}, whence: {})",
			ino, fh, offset, whence
		);
		reply.error(libc::ENOSYS);
	}

	fn copy_file_range(
		&mut self,
		_req: &fuser::Request<'_>,
		ino_in: u64,
		fh_in: u64,
		offset_in: i64,
		ino_out: u64,
		fh_out: u64,
		offset_out: i64,
		len: u64,
		flags: u32,
		reply: fuser::ReplyWrite,
	) {
		debug!(
			"[Not Implemented] copy_file_range(ino_in: {:#x?}, fh_in: {}, \
			offset_in: {}, ino_out: {:#x?}, fh_out: {}, offset_out: {}, \
			len: {}, flags: {})",
			ino_in, fh_in, offset_in, ino_out, fh_out, offset_out, len, flags
		);
		reply.error(libc::ENOSYS);
	}
}


pub type FuseResult<T> = Result<T, FuseError>;

#[derive(Debug, Error)]
pub enum FuseError {
	#[error("Resource not found")]
	NotFound,
	#[error("Data communication failed")]
	Transport,
	#[error("Unknown error")]
	Other,
}

impl From<FuseError> for libc::c_int {
    fn from(t: FuseError) -> Self {
        match t {
			FuseError::NotFound => libc::ENOENT,
            FuseError::Transport => libc::EPIPE,
            FuseError::Other => libc::EINVAL,
		}
    }
}


pub struct Attr {
	pub ino: u64,
	pub size: u64,
	pub kind: FileType,
}

impl From<Metadata> for Attr {
    fn from(meta: Metadata) -> Self {
        Attr { ino: meta.ino, size: meta.size, kind: meta.kind }
    }
}

impl From<Attr> for fuser::FileAttr {
    fn from(attr: Attr) -> Self {
        fuser::FileAttr {
			ino: attr.ino,
			size: attr.size,
			blocks: attr.size,
			atime: SystemTime::UNIX_EPOCH,
			mtime: SystemTime::UNIX_EPOCH,
			ctime: SystemTime::UNIX_EPOCH,
			crtime: SystemTime::UNIX_EPOCH,
			kind: convert_file_type(attr.kind),
			perm: file_type_to_mode(attr.kind) | file_type_default_perms(attr.kind),
			nlink: 0,
			uid: 65535,
			gid: 65535,
			rdev: 0,
			blksize: 1,
			flags: 0,
		}
    }
}


fn convert_file_type(t: FileType) -> fuser::FileType {
	match t {
		FileType::Directory => fuser::FileType::Directory,
		FileType::Regular => fuser::FileType::RegularFile,
	}
}

fn file_type_to_mode(t: FileType) -> u16 {
	match t {
		FileType::Regular => 0o0100000,
		FileType::Directory => 0o0040000,
	}
}

fn file_type_default_perms(t: FileType) -> u16 {
	match t {
		FileType::Regular => 0o644,
		FileType::Directory => 0o755,
	}
}

#[derive(Debug, Clone, Copy)]
pub struct Opened {
	pub fh: u64,
	pub flags: u32,
}
