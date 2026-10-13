# SPDX-License-Identifier: AGPL-3.0-or-later
#
# LinuxCNC 2.9 built from source, run in place, for gate 3 of the G-code
# verification plan (`cargo xtask replay --docker IMAGE`) and any other CI task
# that needs LinuxCNC (OI-47). Nothing here is installed from a LinuxCNC package:
# Ubuntu 24.04 has none.
#
#   docker build -t ghcr.io/paulhampson/openchuckcam-linuxcnc:2.9.10 docker/linuxcnc
#   docker run --rm ghcr.io/paulhampson/openchuckcam-linuxcnc:2.9.10 rs274 --help
#
# The image runs as the unprivileged user `cnc`; LinuxCNC's user-space real-time
# layer needs no privileges for the simulator. Pin a different release with
# --build-arg LINUXCNC_REF=<tag>.
# BASE can point at a mirror of the Ubuntu 24.04 image when Docker Hub rate-limits (429).
ARG BASE=ubuntu:24.04
FROM ${BASE}

ARG LINUXCNC_REF=v2.9.10
ENV DEBIAN_FRONTEND=noninteractive

# Build and run dependencies (from LinuxCNC's debian/control for uspace, less documentation).
RUN apt-get update && apt-get install -y --no-install-recommends \
        build-essential git ca-certificates autoconf automake libtool pkg-config gettext intltool \
        libudev-dev libboost-python-dev libepoxy-dev libgl-dev libglu1-mesa-dev libgtk-3-dev \
        libmodbus-dev libgpiod-dev libedit-dev libeditreadline-dev libtirpc-dev \
        libusb-1.0-0-dev libxmu-dev libxinerama-dev libxft-dev libncurses-dev \
        python3 python3-dev python3-tk python3-xlib python3-gi python3-pil.imagetk python3-yapps yapps2 \
        tcl tcl8.6-dev tclx tk8.6-dev bwidget netpbm psmisc \
        asciidoc docbook-xml docbook-xsl xsltproc \
    && rm -rf /var/lib/apt/lists/*

# `make -k`: the manual pages need tools that are not worth installing, and their failure must not
# stop the build. The check afterwards proves what the simulator needs was built.
RUN git clone --depth 1 --branch "${LINUXCNC_REF}" https://github.com/LinuxCNC/linuxcnc.git /opt/linuxcnc \
    && cd /opt/linuxcnc/src \
    && ./autogen.sh \
    && ./configure --with-realtime=uspace --disable-gtk2 --disable-build-documentation \
    && (make -k -j"$(nproc)" || true) \
    && for f in ../bin/rs274 ../bin/rtapi_app ../bin/milltask ../scripts/linuxcnc ../scripts/rip-environment \
                ../rtlib/motmod.so ../rtlib/trivkins.so ../lib/python/linuxcnc.so; do \
         test -e "$f" || { echo "LinuxCNC build is missing $f" >&2; exit 1; }; \
       done \
    && rm -rf /opt/linuxcnc/.git /opt/linuxcnc/src/objects

COPY linuxcnc-rip /usr/local/bin/linuxcnc
RUN chmod 0755 /usr/local/bin/linuxcnc \
    && for tool in rs274 halcmd; do ln -s /usr/local/bin/linuxcnc /usr/local/bin/$tool; done \
    && useradd --create-home --uid 1000 cnc

ENV LINUXCNC_RIP=/opt/linuxcnc
USER cnc
WORKDIR /work
ENTRYPOINT []
CMD ["linuxcnc", "--help"]
