# Building binaries for the PI (ARM64) with R\*st

[Click me](/foo-my-man)

This post serves as a reminder so I don't forget how to build binaries for the PI, but it can help anyone with the some of the gotchas I had when building [this](https://github.com/biblius/pg_migrator) binary for the PI.

Let's imagine you need a simple CLI tool for your PI. You found no solution that suits you and decided to build your own using R\*st (The crab - in following text). The crab is a very good choice for building CLI tools for 2 reasons; No runtime requirements and a very good ecosystem for your purpose, namely [clap](https://docs.rs/clap/latest/clap/).

## Adding targets to the toolchain

For the crab compiler to be able to compile your code for a specific architecutre, it must be present in the toolchain. [You can find a list of available targets here](https://doc.rust-lang.org/nightly/rustc/platform-support.html). The esoteric targets are known as target triplets (even though they have 4 items!!!) and their format is always `architecture-vendor-os-abi`. Choosing which will depend on the machine in question and its OS.

Adding targets is as simple as

```bash
rustup target add aarch64-unknown-linux-musl

rustup target add aarch64-unknown-linux-gnu
```

To compile it for the target use

```bash
cargo build --release --target=aarch64-unknown-linux-gnu
```

- Note: Before you do this, read until the end to set up the appropriate linker.

For the Orange PI Zero 2, you'll be selecting an `aarch64-unknown-linux` target, but which one will depend on the OS on the pie. [The crab compiler links libraries dynamically by default](https://doc.rust-lang.org/reference/linkage.html#static-and-dynamic-c-runtimes) so you're going to need to investigate whether your pie has them (or just compile statically). When you compile the binary, you can inspect its headers by using

## Check for dependencies

```bash
objdump -p target/release/super_cool_app
```

You are interested in both the dynamic section:

```shutupmdlinter
Dynamic Section:
  NEEDED               libgcc_s.so.1
  NEEDED               libc.so.6
  INIT                 0x0000000000012000
  FINI                 0x00000000000bda20
  ...

  tells you you need libc6
```

and the Version References section:

```shutupmdlinter
Version References:
  required from libgcc_s.so.1:
    0x09276060 0x00 09 GCC_4.2.0
    0x0b792650 0x00 07 GCC_3.0
    0x0b792653 0x00 04 GCC_3.3
  required from libc.so.6:
    0x06969189 0x00 12 GLIBC_2.29
    0x06969185 0x00 11 GLIBC_2.25
    0x06969188 0x00 10 GLIBC_2.28
    0x069691b3 0x00 08 GLIBC_2.33
    0x06969198 0x00 06 GLIBC_2.18
    0x069691b2 0x00 05 GLIBC_2.32
    0x069691b4 0x00 03 GLIBC_2.34
    0x06969197 0x00 02 GLIBC_2.17

    tells you which versions of glibc you need
```

To show the versions of glibc on the pie, run

```bash
objdump -p /usr/lib/aarch64-linux-gnu/libc.so.6
```

and the versions should be in the Version definitions:

```shutupmdlinter
Version definitions:
1 0x01 0x0865f4e6 libc.so.6
2 0x00 0x06969197 GLIBC_2.17
3 0x00 0x06969198 GLIBC_2.18
  GLIBC_2.17
4 0x00 0x06969182 GLIBC_2.22
  GLIBC_2.18
5 0x00 0x06969183 GLIBC_2.23
  GLIBC_2.22
6 0x00 0x06969184 GLIBC_2.24
  GLIBC_2.23
7 0x00 0x06969185 GLIBC_2.25
  GLIBC_2.24
8 0x00 0x06969186 GLIBC_2.26
  GLIBC_2.25
9 0x00 0x06969187 GLIBC_2.27
  GLIBC_2.26
10 0x00 0x06969188 GLIBC_2.28
  GLIBC_2.27
11 0x00 0x06969189 GLIBC_2.29
  GLIBC_2.28
12 0x00 0x069691b0 GLIBC_2.30
  GLIBC_2.29
13 0x00 0x0963cf85 GLIBC_PRIVATE
  GLIBC_2.30
```

If your binary contains versions of GLIBC the pie doesn't, you'll either have to upgrade GLIBC somehow or compile the binary statically.
For the latter, you can use `aarch64-unknown-linux-musl` as the target since that one is linked statically by default, or you can pass in rustflags to the compiler, as shown in the below section. The binary will have everything it needs when it's run and won't need any dependencies from the pie. You can confirm this by running the same `objdump -p` command on the binary created for the musl ABI and you see that its dynamic section is empty.

## The linker

You can check out [all the linkers cargo supports](https://doc.rust-lang.org/rustc/codegen-options/index.html#linker), but for the pie you'll be using `aarch64-linux-gnu-gcc`. We tell which linker the crab compiler will use for which target in the `~/.cargo/config.toml` file (create one if it doesn't exist). The contents should be

```toml
[target.x86_64-unknown-linux-gnu]
linker = "x86_64-linux-gnu-gcc"

[target.aarch64-unknown-linux-gnu]
linker = "aarch64-linux-gnu-gcc"
# The following can be added if you want to statically compile
# rustflags = [
#     "-Ctarget-feature=+crt-static",
# ]

[target.aarch64-unknown-linux-musl]
linker = "aarch64-linux-gnu-gcc"
```

You can now run the [two commands from the start](#adding-targets-to-the-toolchain).

## Libraries

If you can get away with compiling with `musl`, go for it. That one requires the least amount of configuration.

If you are getting an error that the compiler is not found and are on Linux like god intended you can install it with

```bash
sudo apt install gcc-aarch64-linux-gnu
```

When building for different architectures, libraries for those architectures must be present. The `target` arg will only point cargo to the compiler you want it to use, it will not point to the right libraries.
The most notorious library you'll face issues with is OpenSSL.

For example, when compiling a project with `reqwest` as its dependency, it will constantly complain that it cannot find the headers/include directory for OpenSSL. There are 2 things I've found you can do about this:

1. Enable the `native-tls-vendored` feature on reqwest, and
2. Set the `PKG_CONFIG_SYSROOT_DIR` to the appropriate one

Once you've ran the above command, it should've created a directory in `/usr/aarch64-linux-gnu`. This directory contains the necessary libraries to be used for `aarch64-unknown-linux-gnu` and should be set as the sysroot (via `PKG_CONFIG_SYSROOT_DIR`) during compilation.

Now everything should compile and should live happily ever after.

To minimize the size of the binary, [follow this excellent guide](https://github.com/johnthagen/min-sized-rust).
