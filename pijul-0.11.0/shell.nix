with import <nixpkgs> {};

let src = fetchFromGitHub {
      owner = "mozilla";
      repo = "nixpkgs-mozilla";
      rev = "11cf06f0550a022d8bc4850768edecc3beef9f40";
      sha256 = "00fwvvs8qa2g17q4bpwskp3bmis5vac4jp1wsgzcyn64arkxnmys";
   };
in
with import "${src.out}/rust-overlay.nix" pkgs pkgs;

stdenv.mkDerivation {
  name = "rust-pijul";
  buildInputs = [
    rustChannels.stable.rust
    parallel kcov
    python3Packages.httpserver
    libsodium pkgconfig openssl
  ];
}
