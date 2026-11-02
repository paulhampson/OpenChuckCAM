# The OpenChuckCAM user guide

The source of the guide for people who use OpenChuckCAM, published at <https://paulhampson.github.io/OpenChuckCAM/>. It is not the project's engineering documentation: that is [`docs/`](../docs/README.md), which stays the source of truth. [Decision 0400](../docs/decisions/0400-user-documentation-site.md) sets out how the guide is built, versioned and published.

| Path | Contents |
| --- | --- |
| `src/` | The pages, in Markdown; `src/SUMMARY.md` is the contents and their order. |
| `src/tutorial/images/` | The tutorial's screenshots, and `CAPTURED.toml`: the commit they were taken at, and any later review. |
| `theme/` | The page template, the stylesheet and the version script. They replace mdBook's own theme. |
| `tools/build.py` | Builds the guide with mdBook and finishes the pages (version stamp, the G-code notice, fonts). |
| `tools/assemble.py` | Puts the published site together: `latest/` beside every released version. |
| `tools/check.py` | The style, privacy and safety checks, and whether the tutorial's screenshots are older than the screens. |
| `tools/capture.py` | Takes the tutorial's screenshots and the home page's hero images from the real application. |
| `tools/manifest.py` | The SHA-256 manifest of a released version. |
| `tools/tests/` | Tests of the tools. |
| `STYLE.md` | How to write the pages, and the review checklist. |

## Build and check it locally

You need Python 3.11 or later, Git, and the tools pinned in `.github/actions/user-docs/action.yml`: mdBook and lychee (`cargo install mdbook --version 0.5.4 --locked`, `cargo install lychee --version 0.24.2 --locked`), and the Nu HTML Checker with Java. From the repository root:

```text
python3 -m unittest discover -s user-docs/tools/tests
python3 user-docs/tools/build.py --out /tmp/guide/latest-build
python3 user-docs/tools/assemble.py site --latest /tmp/guide/latest-build --out /tmp/guide/root/OpenChuckCAM --base /OpenChuckCAM/
python3 user-docs/tools/check.py --site /tmp/guide/root/OpenChuckCAM
lychee --offline --include-fragments --root-dir /tmp/guide/root /tmp/guide/root/OpenChuckCAM
java -jar vnu.jar --skip-non-html --errors-only /tmp/guide/root/OpenChuckCAM/latest
```

To read it, serve the folder (`python3 -m http.server -d /tmp/guide/root`) and open <http://localhost:8000/OpenChuckCAM/>. The version list needs a server; the pages themselves also open from disk.

## Taking the screenshots

The screenshots come from the real program, never from a mock-up, and show only the repository's example models (`user-docs/STYLE.md`, Figures). `tools/capture.py` builds nothing itself: build the release program first, then run it on Linux with Xvfb, xdotool, ImageMagick, Pillow and a software Vulkan driver (its opening comment lists the packages). Commit any change to the screens first. It writes the images and `src/tutorial/images/CAPTURED.toml`:

```text
cargo build --release -p openchuck-ui
python3 user-docs/tools/capture.py
```

The click positions in the script belong to the layout of one build. When a screen changes, change the positions and the pages that describe it, and capture again. If you check the screens against a newer build without capturing, add `reviewed_at = "<commit>"` to `CAPTURED.toml`. The docs check warns on a pull request when the screens have changed since the capture or review commit, and a release refuses it (decision 0400, section G).

## How it is published

A pull request that changes `user-docs/` runs the checks above as the `User guide` job of CI, part of `CI required checks`. Nothing is published on a push to `main`: `latest/` is the nightly build, deployed by `.github/workflows/docs-site.yml` after each Nightly build that publishes the `nightly` pre-release, from that build's commit. Each released version comes from the `docs-releases` branch, which only the release workflow writes. See [`docs/releasing.md`](../docs/releasing.md) for the release steps and the one-time setup.
