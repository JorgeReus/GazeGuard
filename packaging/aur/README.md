# GazeGuard AUR package

`PKGBUILD` packages the x86_64 Debian release artifact as `gazeguard-bin`.

To test locally:

```sh
cd packaging/aur
makepkg -si
```

To publish, copy `PKGBUILD` into the root of a dedicated AUR repository named
`gazeguard-bin`, update `pkgver` and the checksum for each release, then run:

```sh
makepkg --printsrcinfo > .SRCINFO
git add PKGBUILD .SRCINFO
git commit -m "update package"
git push
```

The application repository does not need `PKGBUILD` at its root unless this
repository itself is also the AUR repository.
