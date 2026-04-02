## Troubleshooting

### Linux / Windows (WSL 2)

> Supported Linux distributions: Ubuntu 22.04, Ubuntu 20.04, Debian 12, Fedora 39

> Supported Windows (WSL 2) distributions: Ubuntu 22.04, Ubuntu 20.04

- **Fedora / Linux:** FireDBG integration tests that assert `i32` **return values** may see **`0` instead of the real value** for very small functions (e.g. `fn f(x: i32) -> i32 { x }`) because LLDB reads the return register before it holds the final result at the `ret` breakpoint. The `call_chain` test allows `0` or `2` for that case on Linux only. This is an environment/LLDB limitation, not a logic bug in the testcase program.

- linker `cc` not found
	```
	error: linker `cc` not found
		|
		= note: No such file or directory (os error 2)
	```

	Try: Open terminal and execute command to install `build-essential`
	```shell
	# Ubuntu
	sudo apt install build-essential
	
	# Debian
	sudo apt install build-essential
	
	# Fedora
	sudo dnf groupinstall "Development Tools"
	
	# CentOS
	sudo yum groupinstall "Development Tools"
	```

- shared library `libc++abi.so` not found
	```
	error while loading shared libraries: libc++abi.so.1
	```

	Try: Open terminal and execute command to install `libc++abi`
	```shell
	# Ubuntu 22.04
	sudo apt install libc++abi1-15
	
	# Ubuntu 20.04
	sudo apt install libc++abi1-10
	
	# Debian 12
	sudo apt install libc++abi1-14
	
	# Debian 10
	sudo apt install libc++abi1-7

	# Fedora 39
	sudo dnf install libcxxabi

	# CentOS Stream 9
	sudo yum install https://kojipkgs.fedoraproject.org//packages/libcxx/17.0.4/1.fc39/x86_64/libcxxabi-17.0.4-1.fc39.x86_64.rpm
	```

- shared library `libunwind.so` not found
	```
	error while loading shared libraries: libunwind.so.1
	```

	Try: Open terminal and execute command to install `libunwind`
	```shell
	sudo apt install libunwind-15
	```

- Link c++ to `clang` instead of g++
	```
	c++: error: unrecognized command-line option '-stdlib=libc++'
	```

	Try: Open terminal and execute command to link c++ to `clang`
	```shell
	sudo update-alternatives --config c++
	```

	On Fedora, current FireDBG `lldb` build scripts compile the C++ bridge with the default `c++` (usually g++) and link `libstdc++`; you should not need to switch alternatives if you are on an up-to-date checkout.

- `unrecognized command-line option '--no-undefined'` (often suggests `-Wno-undef`)
	```
	c++: error: unrecognized command-line option '--no-undefined'
	```

	On Fedora, this often comes from **`CXXFLAGS`** (or `CXXFLAGS_x86_64-unknown-linux-gnu`, etc.) inheriting **linker-only** options from RPM `%optflags` / hardened build defaults. Those are not valid for `c++ -c` (compile only); Clang then suggests `-Wno-undef`.

	Current `lldb` `build.rs` strips common linker-only tokens from `CXXFLAGS*` before calling `cpp_build`. If it still fails:

	```shell
	echo "$CXXFLAGS"
	env | grep '^CXXFLAGS'
	# Temporarily:
	unset CXXFLAGS
	unset CXXFLAGS_x86_64-unknown-linux-gnu

	unset RUSTFLAGS
	cargo clean -p lldb
	cargo build -p firedbg-rust-debugger --features lldb
	```

- `undefined symbol: __gxx_personality_v0` / `__cxa_begin_catch` / `std::terminate()` (from `cpp_closures.cpp`)

	The `lldb` crate embeds a small C++ object file that needs **libstdc++**. Rust’s default link line uses **`-Wl,--as-needed`**, so **lld** can drop **`-lstdc++`** if it appears too early, then fail with these undefined symbols.

	Current `lldb` `build.rs` wraps **`-lstdc++`** with **`-Wl,--push-state,--no-as-needed` … `-Wl,--pop-state`**. If you still see this after `cargo clean -p lldb`, run with **`RUSTFLAGS='-C link-arg=-fuse-ld=bfd'`** (GNU ld instead of lld) once to compare, or report your `ld.lld --version`.

### macOS

> Supported macOS versions: macOS 13 (Ventura), macOS 14 (Sonoma)

- linking with `cc` failed; No developer tools were found
	```
	error: linking with `cc` failed: exit status: 1
		|
		= note: xcode-select: note: No developer tools were found, requesting install.
	```

	Try: Open terminal and execute command to install `Xcode`
	```shell
	xcode-select --install
	```
