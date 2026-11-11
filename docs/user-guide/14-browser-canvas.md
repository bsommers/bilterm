# Chapter 14: The Browser Canvas

`:browser open <url>` shows a real web page inside a pane. Bilterm starts a headless Chrome for
that pane, streams its frames into the pane and forwards your keys and mouse to the page.

## Requirements

- Linux or macOS, and a Chromium-family browser: Chrome, Chromium, Brave or Edge. Bilterm looks
  for `chromium`, `chromium-browser`, `google-chrome-stable`, `google-chrome`, `brave-browser`
  and `microsoft-edge-stable` on `PATH` (plus the standard application paths on macOS). Set
  `BILTERM_CHROME_PATH=/path/to/chrome` to pick one.
- No browser found, or the launch fails: the pane shows a painted placeholder (the *synthetic*
  engine) and the status line says why. On Windows the synthetic engine is always used.

## Opening and closing

| Action | Command | Key | Mouse |
|---|---|---|---|
| Open a URL | `:browser open <url>` or `:browser <url>` | | |
| Toggle the canvas | `:browser` | `Ctrl+Alt+B`, `<Leader> .` | |
| Close | `:browser close` | | `[✕]` on the nav bar |
| Back / forward | `:browser back`, `:browser forward` | | `[◄]` `[►]` |
| Reload / stop | `:browser reload`, `:browser stop` | | `[↻]` (shows `[■]` while loading) |
| Edit the URL | | `Ctrl+L` while the page has focus | click the URL |
| Zoom | `:browser zoom 25`, `:browser zoom -25` | | |
| Give the keyboard to the page | `:browser focus on` | `Ctrl+]` | click the page, or `[FOCUS:…]` |
| Take the keyboard back | `:browser focus off` | `Ctrl+]` | click the prompt, or `[FOCUS:…]` |

`:bcanvas` is an alias of `:browser`. The same operations are available over RPC and MCP as
`browser.open`, `browser.control` and `browser.state` (`bilterm_browser`).

## What the status line tells you

The status line and `browser.state` name the engine that is really running:

| Label | Meaning |
|---|---|
| `cdp (starting)` | Chrome was launched; the DevTools handshake has not finished. |
| `cdp` | Attached to the page. This is real Chrome. |
| `cdp (crashed)` | The page or Chrome died. The pane shows `… — press r to reload`. |
| `synthetic` | Placeholder engine, by request (`demo://`, `test://`, `about:` URLs or the environment switches below). |
| `synthetic (Chrome not found)` | No browser binary was found. |
| `synthetic (launch failed: …)` | Chrome was found but did not come up; the reason follows. A launch that gives no handshake within 5 seconds is reported this way. |

The nav bar shows a lock only for `https://` pages, `◌` while the page is loading, the page
title after the URL, and `⚠` with the reason when a URL was refused or a navigation failed
(for example `net::ERR_NAME_NOT_RESOLVED`).

## Keyboard

Keys go to the floating prompt until the page has focus. Click the page, press `Ctrl+]` or run
`:browser focus on` to give it the keyboard. While it has focus:

- Every key goes to the page, including `Tab`, `Esc`, arrows, `Enter`, function keys and
  `Ctrl`/`Alt` chords. Pasted text is inserted at the page's caret.
- `Ctrl+]` returns the keyboard to bilterm. This always works. (Terminals without the kitty
  keyboard protocol send it as `Ctrl+5`; both are accepted.)
- `Ctrl+L` opens the URL editor: type, `Enter` to go, `Esc` to cancel, `Ctrl+U` to clear. A bare
  host such as `example.com` gets `https://`.
- The leader key (`Ctrl+B` / `Ctrl+X`), your own key bindings, `Ctrl+Q` and `Ctrl+Alt+A` still
  belong to bilterm. So `Ctrl+X` (cut) and `Ctrl+B` are not sent to the page.
- After a crash, `r` relaunches the browser on the last URL (`:browser reload` does the same).

Without focus the page takes no keys. In particular `Ctrl+L` reaches your shell as usual.

## Mouse

Clicks, drags, movement and the wheel on the page are forwarded to it. A click lands where you
point in the terminal and in `--gui`: the position is taken from the size of the frame and of
the pane, not from an assumed cell size. Double clicks are recognised. Clicking the floating
prompt gives the keyboard back to it.

Links that ask for a new window open in the same pane (one page per pane).

## Size and zoom

The page is laid out for the pane: 10 by 20 CSS pixels per cell, between 320x200 and 3840x2160,
and it follows the pane when you resize or split it. `:browser zoom <delta>` changes the zoom in
percent (25% to 500%): the page reflows larger or smaller, as with `Ctrl +` in a desktop browser.

In the terminal the page is drawn with half-blocks, two pixels per cell row, so text on most
pages is not readable there. It is a preview; `bilterm --gui` draws the frame at full size.

## What is allowed

- URLs: `http://`, `https://` and `about:blank`. `javascript:`, `data:`, `chrome:`,
  `view-source:` and anything starting with `-` are refused before they reach Chrome.
- `file://` is off. Start bilterm with `BILTERM_BROWSER_ALLOW_FILE=1` to allow it.
- Downloads are denied.
- Chrome's sandbox stays on. Bilterm never passes `--no-sandbox`; if your system cannot run the
  sandbox (some containers, running as root), Chrome fails to start and you get
  `synthetic (launch failed: …)`.
- Each launch gets its own empty profile in `$XDG_RUNTIME_DIR/bilterm/browser/` (mode `0700`).
  Nothing is shared with your normal browser profile: no cookies, logins or extensions. The
  profile is deleted when the canvas closes, and leftovers from a killed bilterm are removed the
  next time a browser is opened.
- Chrome talks to bilterm over a private pipe. No debugging port is opened.

## Environment switches

| Variable | Effect |
|---|---|
| `BILTERM_CHROME_PATH` | Browser binary to launch. |
| `BILTERM_BROWSER_ALLOW_FILE=1` | Allow `file://` URLs. |
| `BILTERM_BROWSER_SYNTHETIC=1` | Never launch a browser; always use the placeholder engine. |
| `BILTERM_BROWSER_CDP=1` | For tests only: let a test binary launch a real browser. Tests use the placeholder engine otherwise. `BILTERM_BROWSER_CDP=1 cargo test --test browser_cdp_live` runs the live test. |

## Known limits

- One page per pane: no tabs, no find-in-page, no link hints.
- Zooming in lowers the frame resolution (Chrome captures frames at CSS-pixel size).
- No kitty or sixel image output yet, so the terminal view stays a half-block preview.
- Agents cannot yet read page text, take screenshots or click by selector over RPC/MCP.
- The clipboard is the page's own: `Ctrl+C` in the page does not reach the system clipboard.
- JavaScript `alert` dialogs are accepted automatically; `confirm` and `prompt` are dismissed.
