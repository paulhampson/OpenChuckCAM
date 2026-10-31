# SPDX-License-Identifier: AGPL-3.0-or-later
"""Post-build step for the mdBook prototype (lane DOCSITE, throwaway).

mdBook's {{#toc}} helper writes the contents with links relative to the site root and marks no
current page; its own theme fixes both in JavaScript (toc.js). This makes the contents work
without JavaScript: each link is made relative to the page, and the current page gets
aria-current="page".
"""
import os
import re
import sys

root = sys.argv[1]
nav_re = re.compile(r'(<nav class="sidebar".*?</nav>)', re.S)
href_re = re.compile(r'<a href="([^"#:]+\.html)">')

for dirpath, _, files in os.walk(root):
    for name in files:
        if not name.endswith(".html"):
            continue
        path = os.path.join(dirpath, name)
        rel = os.path.relpath(path, root).replace(os.sep, "/")
        depth = rel.count("/")
        prefix = "../" * depth
        with open(path, encoding="utf-8") as f:
            html = f.read()

        def fix_nav(m):
            def fix_link(a):
                target = a.group(1)
                current = ' aria-current="page"' if target == rel else ""
                return f'<a href="{prefix}{target}"{current}>'
            return href_re.sub(fix_link, m.group(1))

        new = nav_re.sub(fix_nav, html, count=1)
        if new != html:
            with open(path, "w", encoding="utf-8") as f:
                f.write(new)
