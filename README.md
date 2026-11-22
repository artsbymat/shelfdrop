# Shelfdrop

Shelfdrop adalah panel desktop Linux Wayland untuk menaruh referensi berkas sementara. Seret berkas dari file manager ke panel, lalu seret item yang dipilih ke folder atau aplikasi tujuan. Drag keluar menawarkan salin. Remove dan Clear All melepas referensi dari shelf.

Gambar juga bisa diseret langsung dari halaman browser. Shelfdrop memakai data gambar yang diberikan browser atau mengunduh URL gambar HTTP/HTTPS secara anonim. Gambar raster hingga 16 MiB disimpan di `$XDG_CACHE_HOME/shelfdrop/browser-images/` (default `~/.cache/shelfdrop/browser-images/`) agar bisa dipratinjau, disalin, dan diseret keluar seperti berkas lokal. Aksi pembersihan shelf melepas referensinya. Salinan cache tetap tersedia untuk aplikasi yang sudah menerima berkas.

## Jalankan dari source

Pasang Rust dan Cargo, kompiler sistem, `pkg-config`, serta GTK 4.8 atau lebih baru dengan paket pengembangan dan pemuat SVG GdkPixbuf. Pada Arch Linux paket utamanya adalah `rust`, `base-devel`, `gtk4`, dan `librsvg`. Pada Debian atau Ubuntu gunakan toolchain Rust terbaru bersama `build-essential`, `pkg-config`, `libgtk-4-dev`, dan `librsvg2-common`.

```sh
GDK_BACKEND=wayland cargo run --locked
```

Tekan Shift+F10 untuk menu konteks, Enter untuk membuka berkas, Ctrl+C untuk menyalin pilihan, atau Delete untuk melepas item. Pilih Compact atau Expanded di Settings. Penempatan dan stacking mengikuti compositor. Settings menerapkan preset layout langsung dan menyimpan ukuran yang diminta untuk pembukaan berikutnya. Perubahan ukuran jendela saat sedang terbuka bergantung pada compositor. Pada compositor tiling, gunakan kontrol compositor untuk mengambangkan jendela.

## Pasang untuk pengguna lokal

```sh
cargo build --release --locked
install -Dm755 target/release/shelfdrop ~/.local/bin/shelfdrop
install -Dm644 data/io.github.shelfdrop.Shelfdrop.desktop ~/.local/share/applications/io.github.shelfdrop.Shelfdrop.desktop
install -Dm644 data/io.github.shelfdrop.Shelfdrop.svg ~/.local/share/icons/hicolor/scalable/apps/io.github.shelfdrop.Shelfdrop.svg
```

Pastikan `~/.local/bin` ada di PATH sesi desktop agar peluncur dapat menemukan biner. Ikon A1 dan ilustrasi drop bersumber dari [desain Shelfdrop](../shelfdrop-design/). Ikon jenis berkas berasal dari tema ikon desktop saat aplikasi berjalan.

## Verifikasi

```sh
cargo fmt --check
cargo clippy --all-targets -- -D warnings
cargo test
```

Lihat [PLAN.md](PLAN.md) untuk status pengujian Wayland.
