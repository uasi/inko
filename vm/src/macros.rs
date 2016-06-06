#![macro_use]

/// Acquires a read lock from an RwLock.
macro_rules! read_lock {
    ($value: expr) => (
        $value.read().unwrap()
    );
}

/// Acquires a write lock from an RwLock
macro_rules! write_lock {
    ($value: expr) => (
        $value.write().unwrap()
    );
}

/// Calls an instruction method on a given receiver.
macro_rules! run {
    ($rec: expr, $name: ident, $process: ident, $code: ident, $ins: ident) => (
        try!($rec.$name($process.clone(), $code.clone(), &$ins));
    );
}

/// Returns an Err if any of the given arguments is not an integer.
macro_rules! ensure_integers {
    ($($ident: ident),+) => (
        $(
            if !$ident.value.is_integer() {
                return Err("all arguments must be Integer objects".to_string());
            }
        )+
    );
}

/// Returns an Err if any of the given arguments is not a float.
macro_rules! ensure_floats {
    ($($ident: ident),+) => (
        $(
            if !$ident.value.is_float() {
                return Err("all arguments must be Float objects".to_string());
            }
        )+
    );
}

/// Returns an Err if any of the given arguments is not an array.
macro_rules! ensure_arrays {
    ($($ident: ident),+) => (
        $(
            if !$ident.value.is_array() {
                return Err("all arguments must be Array objects".to_string());
            }
        )+
    );
}

/// Returns an Err if any of the given arguments is not a string.
macro_rules! ensure_strings {
    ($($ident: ident),+) => (
        $(
            if !$ident.value.is_string() {
                return Err("all arguments must be String objects".to_string());
            }
        )+
    );
}

/// Returns an Err if any of the given arguments is not a file.
macro_rules! ensure_files {
    ($($ident: ident),+) => (
        $(
            if !$ident.value.is_file() {
                return Err("all arguments must be File objects".to_string());
            }
        )+
    );
}

/// Returns an Err if any of the given arguments is not a CompiledCode value.
macro_rules! ensure_compiled_code {
    ($($ident: ident),+) => (
        $(
            if !$ident.value.is_compiled_code() {
                return Err("all arguments must be CompiledCode objects".to_string());
            }
        )+
    );
}

macro_rules! to_expr {
    ($e: expr) => ($e);
}

/// Returns an RcObject from a thread using an instruction argument.
macro_rules! instruction_object {
    ($ins: expr, $process: expr, $index: expr) => ({
        let index = try!($ins.arg($index));
        let lock = read_lock!($process);

        try!(lock.get_register(index))
    });
}

/// Returns a vector index for an i64
macro_rules! int_to_vector_index {
    ($vec: expr, $index: expr) => ({
        if $index >= 0 as i64 {
            $index as usize
        }
        else {
            ($vec.len() as i64 - $index) as usize
        }
    });
}

/// Ensures the given index is within the bounds of the array.
macro_rules! ensure_array_within_bounds {
    ($array: ident, $index: expr) => (
        if $index >= $array.len() {
            return Err(format!("index {} is out of bounds", $index));
        }
    );
}

/// Ensures the given number of bytes to read is greater than 0
macro_rules! ensure_positive_read_size {
    ($size: expr) => (
        if $size < 0 {
            return Err("can't read a negative amount of bytes".to_string());
        }
    );
}

/// Returns a string to use for reading from a file, optionally with a max size.
macro_rules! file_reading_buffer {
    ($instruction: ident, $process: ident, $size_idx: expr) => (
        if $instruction.arguments.get($size_idx).is_some() {
            let size_ptr = instruction_object!($instruction, $process,
                                               $size_idx);

            let size_ref = size_ptr.get();
            let size_obj = size_ref.get();

            ensure_integers!(size_obj);

            let size = size_obj.value.as_integer();

            ensure_positive_read_size!(size);

            String::with_capacity(size as usize)
        }
        else {
            String::new()
        }
    );
}

/// Sets an error in a register and returns control to the caller.
macro_rules! set_error {
    ($code: expr, $process: expr, $register: expr) => ({
        let mut lock = write_lock!($process);
        let obj = lock.allocate_without_prototype(object_value::error($code));

        lock.set_register($register, obj);

        return Ok(());
    });
}

/// Returns a Result's OK value or stores the error in a register.
macro_rules! try_error {
    ($expr: expr, $process: expr, $register: expr) => (
        match $expr {
            Ok(val)   => val,
            Err(code) => set_error!(code, $process, $register)
        }
    );
}

/// Returns a Result's OK value or stores an IO error in a register.
macro_rules! try_io {
    ($expr: expr, $process: expr, $register: expr) => (
        try_error!($expr.map_err(|err| errors::from_io_error(err)), $process,
                   $register)
    );
}

/// Tries to create a String from a collection of bytes.
macro_rules! try_from_utf8 {
    ($bytes: expr) => (
        String::from_utf8($bytes).map_err(|_| errors::STRING_INVALID_UTF8 )
    );
}

macro_rules! constant_error {
    ($reg: expr, $name: expr) => (
        format!(
            "The object in register {} does not define the constant \"{}\"",
            $reg,
            $name
        )
    )
}

macro_rules! attribute_error {
    ($reg: expr, $name: expr) => (
        format!(
            "The object in register {} does not define the attribute \"{}\"",
            $reg,
            $name
        );
    )
}

macro_rules! copy_if_global {
    ($heap: expr, $source: expr, $dest: expr) => ({
        if $dest.is_global() {
            write_lock!($heap).copy_object($source)
        }
        else {
            $source
        }
    });
}

macro_rules! num_op {
    ($vm: expr, $process: expr, $ins: expr, $op: tt, $tname: ident, $as_name: ident, $ensure: ident, $proto: ident) => ({
        let register = try!($ins.arg(0));
        let receiver_ptr = instruction_object!($ins, $process, 1);
        let arg_ptr = instruction_object!($ins, $process, 2);

        let receiver_ref = receiver_ptr.get();
        let receiver = receiver_ref.get();

        let arg_ref = arg_ptr.get();
        let arg = arg_ref.get();

        $ensure!(receiver, arg);

        let result = to_expr!(receiver.value.$as_name() $op arg.value.$as_name());

        let obj = write_lock!($process)
            .allocate(object_value::$tname(result), $vm.$proto.clone());

        write_lock!($process).set_register(register, obj);
    });
}

macro_rules! num_bool_op {
    ($vm: expr, $process: expr, $ins: expr, $op: tt, $as_name: ident, $ensure: ident) => ({
        let register = try!($ins.arg(0));
        let receiver_ptr = instruction_object!($ins, $process, 1);
        let arg_ptr = instruction_object!($ins, $process, 2);

        let receiver_ref = receiver_ptr.get();
        let receiver = receiver_ref.get();

        let arg_ref = arg_ptr.get();
        let arg = arg_ref.get();

        $ensure!(receiver, arg);

        let result = to_expr!(receiver.value.$as_name() $op arg.value.$as_name());

        let boolean = if result {
            $vm.true_object.clone()
        }
        else {
            $vm.false_object.clone()
        };

        write_lock!($process).set_register(register, boolean);
    });
}

macro_rules! integer_op {
    ($vm: expr, $process: expr, $ins: expr, $op: tt) => ({
        num_op!($vm, $process, $ins, $op, integer, as_integer, ensure_integers,
                integer_prototype);
    });
}

macro_rules! integer_bool_op {
    ($vm: expr, $process: expr, $ins: expr, $op: tt) => ({
        num_bool_op!($vm, $process, $ins, $op, as_integer, ensure_integers);
    });
}

macro_rules! float_op {
    ($vm: expr, $process: expr, $ins: expr, $op: tt) => ({
        num_op!($vm, $process, $ins, $op, float, as_float, ensure_floats,
                float_prototype);
    });
}

macro_rules! float_bool_op {
    ($vm: expr, $process: expr, $ins: expr, $op: tt) => ({
        num_bool_op!($vm, $process, $ins, $op, as_float, ensure_floats);
    });
}
