//! VM functions for working with IO.
use crate::filesystem;
use crate::object_pointer::ObjectPointer;
use crate::object_value;
use crate::process::RcProcess;
use crate::runtime_error::RuntimeError;
use crate::vm::state::RcState;
use num_traits::ToPrimitive;
use std::fs;
use std::fs::OpenOptions;
use std::io::{self, Read, Seek, SeekFrom, Write};

/// File opened for reading, equal to fopen's "r" mode.
const READ: i64 = 0;

/// File opened for writing, equal to fopen's "w" mode.
const WRITE: i64 = 1;

/// File opened for appending, equal to fopen's "a" mode.
const APPEND: i64 = 2;

/// File opened for both reading and writing, equal to fopen's "w+" mode.
const READ_WRITE: i64 = 3;

/// File opened for reading and appending, equal to fopen's "a+" mode.
const READ_APPEND: i64 = 4;

macro_rules! file_mode_error {
    ($mode: expr) => {
        return Err(format!("Invalid file open mode: {}", $mode));
    };
}

/// Reads a number of bytes from a stream into a byte array.
pub fn io_read(
    state: &RcState,
    process: &RcProcess,
    stream: &mut Read,
    buffer: &mut Vec<u8>,
    amount: ObjectPointer,
) -> Result<ObjectPointer, RuntimeError> {
    let result = if amount.is_integer() {
        let amount_bytes = amount.usize_value()?;

        stream.take(amount_bytes as u64).read_to_end(buffer)?
    } else {
        stream.read_to_end(buffer)?
    };

    // When reading into a buffer, the Vec type may decide to grow it beyond the
    // necessary size. This can lead to a waste of memory, especially when the
    // buffer only sticks around for a short amount of time. To work around this
    // we manually shrink the buffer once we're done writing.
    buffer.shrink_to_fit();

    Ok(process.allocate_usize(result, state.integer_prototype))
}

#[cfg_attr(feature = "cargo-clippy", allow(trivially_copy_pass_by_ref))]
pub fn buffer_to_write(buffer: &ObjectPointer) -> Result<&[u8], RuntimeError> {
    let buff = if buffer.is_string() {
        buffer.string_value()?.as_bytes()
    } else {
        buffer.byte_array_value()?
    };

    Ok(buff)
}

pub fn io_write<W: Write>(
    state: &RcState,
    process: &RcProcess,
    output: &mut W,
    to_write: ObjectPointer,
) -> Result<ObjectPointer, RuntimeError> {
    let written = output.write(buffer_to_write(&to_write)?)?;

    Ok(process.allocate_usize(written, state.integer_prototype))
}

pub fn io_flush<W: Write>(
    state: &RcState,
    output: &mut W,
) -> Result<ObjectPointer, RuntimeError> {
    Ok(output.flush().map(|_| state.nil_object)?)
}

pub fn stdout_write(
    state: &RcState,
    process: &RcProcess,
    to_write: ObjectPointer,
) -> Result<ObjectPointer, RuntimeError> {
    let mut output = io::stdout();

    io_write(state, process, &mut output, to_write)
}

pub fn stdout_flush(state: &RcState) -> Result<ObjectPointer, RuntimeError> {
    let mut output = io::stdout();

    io_flush(state, &mut output)
}

pub fn stderr_write(
    state: &RcState,
    process: &RcProcess,
    to_write: ObjectPointer,
) -> Result<ObjectPointer, RuntimeError> {
    let mut output = io::stderr();

    io_write(state, process, &mut output, to_write)
}

pub fn stderr_flush(state: &RcState) -> Result<ObjectPointer, RuntimeError> {
    let mut output = io::stdout();

    io_flush(state, &mut output)
}

pub fn stdin_read(
    state: &RcState,
    process: &RcProcess,
    buffer_ptr: ObjectPointer,
    amount: ObjectPointer,
) -> Result<ObjectPointer, RuntimeError> {
    let mut input = io::stdin();
    let buffer = buffer_ptr.byte_array_value_mut()?;

    io_read(state, process, &mut input, buffer, amount)
}

pub fn write_file(
    state: &RcState,
    process: &RcProcess,
    file_ptr: ObjectPointer,
    to_write: ObjectPointer,
) -> Result<ObjectPointer, RuntimeError> {
    let file = file_ptr.file_value_mut()?;

    io_write(state, process, file, to_write)
}

pub fn flush_file(
    state: &RcState,
    file_ptr: ObjectPointer,
) -> Result<ObjectPointer, RuntimeError> {
    let file = file_ptr.file_value_mut()?;

    io_flush(state, file)
}

pub fn read_file(
    state: &RcState,
    process: &RcProcess,
    file_ptr: ObjectPointer,
    buffer_ptr: ObjectPointer,
    amount: ObjectPointer,
) -> Result<ObjectPointer, RuntimeError> {
    let mut input = file_ptr.file_value_mut()?;
    let buffer = buffer_ptr.byte_array_value_mut()?;

    io_read(state, process, &mut input, buffer, amount)
}

pub fn open_file(
    state: &RcState,
    process: &RcProcess,
    path_ptr: ObjectPointer,
    mode_ptr: ObjectPointer,
) -> Result<ObjectPointer, RuntimeError> {
    let path = path_ptr.string_value()?;
    let mode = mode_ptr.integer_value()?;
    let open_opts = options_for_integer(mode)?;
    let prototype = prototype_for_open_mode(&state, mode)?;
    let file = open_opts.open(path)?;

    Ok(process.allocate(object_value::file(file), prototype))
}

pub fn file_size(
    state: &RcState,
    process: &RcProcess,
    path_ptr: ObjectPointer,
) -> Result<ObjectPointer, RuntimeError> {
    let path = path_ptr.string_value()?;
    let meta = fs::metadata(path)?;

    Ok(process.allocate_u64(meta.len(), state.integer_prototype))
}

pub fn seek_file(
    state: &RcState,
    process: &RcProcess,
    file_ptr: ObjectPointer,
    offset_ptr: ObjectPointer,
) -> Result<ObjectPointer, RuntimeError> {
    let file = file_ptr.file_value_mut()?;

    let offset = if offset_ptr.is_bigint() {
        let big_offset = offset_ptr.bigint_value()?;

        if let Some(offset) = big_offset.to_u64() {
            offset
        } else {
            return Err(RuntimeError::Panic(format!(
                "{} is too big for a seek offset",
                big_offset
            )));
        }
    } else {
        let offset = offset_ptr.integer_value()?;

        if offset < 0 {
            return Err(RuntimeError::Panic(format!(
                "{} is not a valid seek offset",
                offset
            )));
        }

        offset as u64
    };

    let cursor = file.seek(SeekFrom::Start(offset))?;

    Ok(process.allocate_u64(cursor, state.integer_prototype))
}

pub fn remove_file(
    state: &RcState,
    path_ptr: ObjectPointer,
) -> Result<ObjectPointer, RuntimeError> {
    let path_str = path_ptr.string_value()?;

    fs::remove_file(path_str)?;

    Ok(state.nil_object)
}

pub fn copy_file(
    state: &RcState,
    process: &RcProcess,
    src_ptr: ObjectPointer,
    dst_ptr: ObjectPointer,
) -> Result<ObjectPointer, RuntimeError> {
    let src = src_ptr.string_value()?;
    let dst = dst_ptr.string_value()?;
    let bytes_copied = fs::copy(src, dst)?;

    Ok(process.allocate_u64(bytes_copied, state.integer_prototype))
}

pub fn file_type(
    path_ptr: ObjectPointer,
) -> Result<ObjectPointer, RuntimeError> {
    let path = path_ptr.string_value()?;
    let file_type = filesystem::type_of_path(path);

    Ok(ObjectPointer::integer(file_type))
}

pub fn file_time(
    state: &RcState,
    process: &RcProcess,
    path_ptr: ObjectPointer,
    kind_ptr: ObjectPointer,
) -> Result<ObjectPointer, RuntimeError> {
    let path = path_ptr.string_value()?;
    let kind = kind_ptr.integer_value()?;
    let dt = filesystem::date_time_for_path(path, kind)?;

    Ok(process
        .allocate(object_value::float(dt.timestamp()), state.float_prototype))
}

pub fn options_for_integer(mode: i64) -> Result<OpenOptions, String> {
    let mut open_opts = OpenOptions::new();

    match mode {
        READ => open_opts.read(true),
        WRITE => open_opts.write(true).truncate(true).create(true),
        APPEND => open_opts.append(true).create(true),
        READ_WRITE => open_opts.read(true).write(true).create(true),
        READ_APPEND => open_opts.read(true).append(true).create(true),
        _ => file_mode_error!(mode),
    };

    Ok(open_opts)
}

pub fn prototype_for_open_mode(
    state: &RcState,
    mode: i64,
) -> Result<ObjectPointer, String> {
    let proto = match mode {
        READ => state.read_only_file_prototype,
        WRITE | APPEND => state.write_only_file_prototype,
        READ_WRITE | READ_APPEND => state.read_write_file_prototype,
        _ => file_mode_error!(mode),
    };

    Ok(proto)
}

pub fn create_directory(
    state: &RcState,
    path_ptr: ObjectPointer,
    recursive_ptr: ObjectPointer,
) -> Result<ObjectPointer, RuntimeError> {
    let path = path_ptr.string_value()?;

    if is_false!(state, recursive_ptr) {
        fs::create_dir(path)?;
    } else {
        fs::create_dir_all(path)?;
    }

    Ok(state.nil_object)
}

pub fn remove_directory(
    state: &RcState,
    path_ptr: ObjectPointer,
    recursive_ptr: ObjectPointer,
) -> Result<ObjectPointer, RuntimeError> {
    let path = path_ptr.string_value()?;

    if is_false!(state, recursive_ptr) {
        fs::remove_dir(path)?;
    } else {
        fs::remove_dir_all(path)?;
    }

    Ok(state.nil_object)
}

pub fn list_directory(
    state: &RcState,
    process: &RcProcess,
    path_ptr: ObjectPointer,
) -> Result<ObjectPointer, RuntimeError> {
    let path = path_ptr.string_value()?;
    let files = filesystem::list_directory_as_pointers(&state, process, path)?;

    Ok(files)
}