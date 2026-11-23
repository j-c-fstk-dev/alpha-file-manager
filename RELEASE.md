# Alpha File Manager v0.3.0

- Redesigned the interface: header with a search field (Ctrl+K), brand block, quieter menu bar,
  and a primary New Tab button; sidebar with line icons; home cards with item counts, a wide
  pinned card, and a disk usage card; three button styles and a new app icon.
- Added an icon view next to the list view, with image thumbnails.
- Made large folders load much faster: folders are read in the background and the listing is
  built without blocking the window. Filtering and sorting no longer reread the disk.
- Fixed the color theme failing to load on GTK 3 because of unsupported CSS values.

The Linux x86-64 archive includes the executable and a per-user launcher installer.
It requires GTK 3 and a compatible glibc. See the README for source builds.
