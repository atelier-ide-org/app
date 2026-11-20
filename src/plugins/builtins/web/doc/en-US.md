---
name: Web viewer
summary: Web pages in the editor, as in a browser, and the page of an HTML file next to its source.
---

# Web viewer

**Web: open a page** (in the palette) opens a browser tab: an address bar, the back, forward and reload buttons, the developer tools, and a button that opens the page in the system browser. In the bar, an address without `https://` is enough (`localhost:3000` goes over HTTP), a path opens the file, and words search. A link that opens a new window opens a new tab. An address typed in **Search Everywhere** (Shift Shift) offers to open it in the viewer, and Shift Shift works from the page too.

A page is a `.url` file, an Internet shortcut as Windows makes them: those of the project open in the viewer too. A page opened from the palette opens again where it was at the next start, and goes away with its tab.

An HTML file shows its page next to its source with the preview button at the top right of the editor, or **Web: page of the HTML file next to its source**. It reloads at each save. **Open in the Web viewer**, in the menu of the tree, opens it in a tab of its own.

The viewer uses the web engine of the system (WebView2 on Windows, WebKit on macOS), with cookies and data of its own, kept by the IDE. It is not available on Linux yet. A page shows in the main window only, and hides while a menu or a floating window passes over it.
