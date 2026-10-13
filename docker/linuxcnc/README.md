# LinuxCNC image for CI

`Dockerfile` builds LinuxCNC 2.9.10 from source (Ubuntu 24.04 has no package), run in place, as the unprivileged user `cnc`. CI uses it for gate 3 (`cargo xtask replay --docker`) and any other task that needs LinuxCNC (decision record 0020 item 13, OI-47).

```
docker build -t ghcr.io/paulhampson/openchuckcam-linuxcnc:2.9.10 docker/linuxcnc
cargo xtask replay --docker ghcr.io/paulhampson/openchuckcam-linuxcnc:2.9.10
```

The image is published by `.github/workflows/linuxcnc-image.yml`. `linuxcnc`, `rs274` and `halcmd` are on the PATH (they source LinuxCNC's `rip-environment` first). Behind a registry rate limit, `--build-arg BASE=mirror.gcr.io/library/ubuntu:24.04`.

The build takes about 20 minutes. The manual pages are not built; the Dockerfile fails the build if a file the simulator needs is missing. Licence: AGPL-3.0-or-later for the files here; LinuxCNC itself is GPL-2.0-or-later and is only built into the image, never linked into OpenChuckCAM.
