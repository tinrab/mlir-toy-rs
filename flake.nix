{
  description = "dev";

  inputs = {
    nixpkgs.url = "github:NixOS/nixpkgs/nixos-unstable";
    flake-utils.url = "github:numtide/flake-utils";
  };

  outputs =
    {
      self,
      nixpkgs,
      flake-utils,
    }:
    flake-utils.lib.eachDefaultSystem (
      system:
      let
        pkgs = import nixpkgs { inherit system; };
        lib = pkgs.lib;

        # Single knob for the toolchain version. melior 0.28 (mlir-sys 230) needs LLVM 23.
        # Bump together with the `melior` version in Cargo.toml.
        llvmPkgs = pkgs.llvmPackages_23;

        # Use the Clang stdenv so `cc`/`c++`/CMake default to clang, not gcc.
        mkShell = pkgs.mkShell.override { stdenv = llvmPkgs.stdenv; };

        llvmDev = lib.getDev llvmPkgs.llvm;
        mlirDev = lib.getDev llvmPkgs.mlir;
        llvmLib = lib.getLib llvmPkgs.llvm;

        # `mlir-sys` (the FFI layer under melior) finds LLVM *and* MLIR through a
        # single `llvm-config`, assuming both are installed under one prefix.
        # Nix splits them into separate store paths, so merge them into one tree
        # and wrap `llvm-config` so its include/lib directories point into it.
        # The build scripts find this prefix through `MLIR_SYS_230_PREFIX`.
        llvmMlirPrefix = pkgs.symlinkJoin {
          name = "llvm-mlir-${llvmPkgs.llvm.version}";
          paths = [
            llvmLib
            llvmDev
            llvmPkgs.mlir
            mlirDev
          ];
          postBuild = ''
            rm "$out/bin/llvm-config"
            cat > "$out/bin/llvm-config" <<EOF
            #!${pkgs.runtimeShell}
            set -o pipefail
            ${llvmDev}/bin/llvm-config "\$@" \
              | sed -e 's|${llvmDev}/include|$out/include|g' -e 's|${llvmLib}/lib|$out/lib|g'
            EOF
            chmod +x "$out/bin/llvm-config"
          '';
        };

        pythonEnv = pkgs.python3.withPackages (ps: [
          ps.lit # LLVM/MLIR test runner
          ps.numpy
          ps.pyyaml
          ps.pybind11
          ps.nanobind
        ]);

        llvmTools = [
          # clang-tools first so its wrapped clangd/clang-tidy/clang-format
          # (which know the Nix include paths) take precedence on PATH.
          llvmPkgs.clang-tools
          llvmPkgs.clang
          llvmPkgs.llvm # llvm-config, opt, llc, llvm-objdump, ...
          llvmDev # LLVMConfig.cmake, headers
          llvmPkgs.mlir # mlir-opt, mlir-translate, mlir-tblgen, ...
          mlirDev # MLIRConfig.cmake, headers
          llvmPkgs.lld
          llvmPkgs.lldb
          llvmPkgs.libllvm
          llvmPkgs.bintools
        ];

        buildTools = with pkgs; [
          cmake
          ninja
          meson
          gnumake
          pkg-config
          ccache
          bear
          just
          git
        ];

        analysisTools =
          with pkgs;
          [
            cppcheck
            include-what-you-use
            doxygen
            graphviz
            hyperfine
            nixfmt
          ]
          ++ lib.optionals stdenv.hostPlatform.isLinux [
            gdb
            valgrind
            linuxPackages.perf
          ];

        commonLibs = with pkgs; [
          zlib
          zstd
          libxml2
          ncurses
        ];
      in
      {
        devShells.default = mkShell {
          name = "dev";

          # Let -O0 debug builds work without _FORTIFY_SOURCE warnings.
          hardeningDisable = [ "fortify" ];

          packages = llvmTools ++ buildTools ++ analysisTools ++ commonLibs ++ [ pythonEnv ];

          shellHook = ''
            export CC=clang
            export CXX=clang++

            export LLVM_DIR="${llvmDev}/lib/cmake/llvm"
            export MLIR_DIR="${mlirDev}/lib/cmake/mlir"
            export MLIR_SYS_230_PREFIX="${llvmMlirPrefix}"
            export CLANG_DIR="${lib.getDev llvmPkgs.libclang}/lib/cmake/clang"
            export LIBCLANG_PATH="${lib.getLib llvmPkgs.libclang}/lib"

            # Make find_package(LLVM/MLIR/Clang) work without extra -D flags.
            export CMAKE_PREFIX_PATH="${llvmDev}:${mlirDev}:${lib.getDev llvmPkgs.libclang}''${CMAKE_PREFIX_PATH:+:$CMAKE_PREFIX_PATH}"

            # clangd picks this up; also a sane default generator.
            export CMAKE_EXPORT_COMPILE_COMMANDS=1
            export CMAKE_GENERATOR=Ninja

            export CCACHE_DIR="''${CCACHE_DIR:-$PWD/.ccache}"
          '';
        };

        formatter = pkgs.nixfmt;

        apps = {
          format = {
            type = "app";
            program = "${pkgs.writeShellScript "format" ''
              set -euo pipefail

              prune=( -path ./.git -o -path ./.direnv -o -path ./.ccache -o -path './build*' -o -path ./result )

              echo "Formatting Nix files..."
              find . \( "''${prune[@]}" \) -prune -o -type f -name '*.nix' -print0 \
                | xargs -0 -r ${pkgs.nixfmt}/bin/nixfmt

              echo "Formatting C/C++ files..."
              find . \( "''${prune[@]}" \) -prune -o -type f \( \
                  -name '*.c' -o -name '*.h' -o \
                  -name '*.cc' -o -name '*.cpp' -o -name '*.cxx' -o \
                  -name '*.hh' -o -name '*.hpp' -o -name '*.hxx' -o \
                  -name '*.inc' -o -name '*.ipp' -o -name '*.tpp' \
                \) -print0 \
                | xargs -0 -r ${llvmPkgs.clang-tools}/bin/clang-format -i --style=file --fallback-style=LLVM
            ''}";
            meta.description = "Format Nix and C/C++ files";
          };
        };
      }
    );
}
