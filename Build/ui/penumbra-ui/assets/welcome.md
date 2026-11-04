# Hello, Penumbra

This is your first note. Everything you write is plain Markdown, and it lives on a map instead of in a list.

## Rename a note

Double-click a card to open it. The title sits in the bar at the top of the editor, and it's a text field: click it, type the new name, then click back into the body. The title is also the filename in your vault, so renaming the note renames the file, and any `[[wikilinks]]` pointing at the old title are rewritten for you.

## New notes

The New note button in the bottom-right corner drops a note called Untitled somewhere on the map. It doesn't open itself, so double-click it, then rename it so it means something.

## Moving around

Left-drag a card to shove it around. When you let go it drifts back to where the layout thinks it belongs, because the map sorts itself by what the notes are similar to.
Right-drag a card to pin it in place. A pinned note stays exactly where you put it and the notes near it settle around it; right-click it again to unpin and let it drift.
Left-drag empty space to pan, and scroll to pan too. Hold Cmd (Ctrl on Windows and Linux) while scrolling to zoom.
Alt-drag from one card onto another to link them. That appends a `[[link]]` to the first note, and the two cards end up near each other.

## Finding things

The Quick finder in the top bar takes a phrase; press Enter and the camera glides to the closest note and selects it. With the semantic model loaded it ranks by meaning, otherwise it falls back to matching characters.

## Tags

Drop `#tags` in the body and they show up as colored dots on the card. Tags can use letters, digits, `-`, `_`, and `/` for nesting, like `#projects/penumbra`; a tag that is only digits is ignored. Tags in a `tags:` frontmatter field are merged in as well.

## In the editor

It's Markdown, so `#` makes a heading and the usual syntax works, just with the markers dimmed instead of hidden. Cmd+B bolds and Cmd+I italicizes. Cmd+C/X/V copy, cut, and paste; Cmd+A selects everything; Cmd+Z and Cmd+Shift+Z undo and redo. Alt+Left and Alt+Right move by word, and Tab and Shift+Tab indent and outdent. Press Esc to close the note and glide back out to the map.
