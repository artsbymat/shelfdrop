# Panduan Agen Shelfdrop

Shelfdrop adalah aplikasi desktop Linux Wayland. Gunakan Rust sebagai bahasa utama, Cargo sebagai toolchain, dan GTK4/GDK/GIO untuk integrasi desktop. Baca [PRD.md](PRD.md) untuk perilaku produk dan [PLAN.md](PLAN.md) untuk urutan implementasi.

Untuk perubahan UI, baca mockup di `../shelfdrop-design/` relatif terhadap root repo aplikasi. [Folder desain](../shelfdrop-design/) · [FigCraft](../shelfdrop-design/shelfdrop.figcraft) · [Pratinjau](../shelfdrop-design/shelfdrop-preview.png)

## Cara bekerja

- Ikuti pola repo begitu tersedia. Pada repo kosong, mulai dengan satu crate aplikatif. Pisahkan logika daftar berkas dari widget agar aturan produk dapat diuji lewat `cargo test`.
- Buat perubahan sekecil mungkin yang menyelesaikan alur pengguna. Pakai API toolkit dan dependensi yang sudah ada sebelum menambah pustaka. Pertahankan perilaku pemanggil dan data sesi lama saat mengubah format.
- Gunakan API GIO dan representasi path/URI yang mempertahankan encoding. Validasi input eksternal di batas masuk. Pertahankan UI responsif saat membaca metadata. Jangan interpolasi URI atau path ke perintah shell.
- Shelf memiliki referensi berkas. Remove, Clear All, drag salin keluar, dan clear on exit tidak boleh menghapus atau memindahkan berkas asli. Pastikan retry, batal, dan drop yang berdekatan tidak menggandakan item atau merusak state.
- Hormati kontrol compositor. Gunakan fallback System default untuk posisi dan stacking. Tampilkan opsi yang bergantung pada compositor hanya sesuai kemampuan yang sudah diverifikasi.
- Pertahankan logo A1 Sela Lembut, ikon berkas empty state yang terisi, serta keterbacaan tema gelap dan terang. Pastikan kontrol punya nama yang dapat diakses, fokus jelas, dan dapat digunakan lewat keyboard tanpa drag-and-drop.

## Verifikasi perubahan

Setelah crate tersedia, jalankan perintah yang relevan dengan perubahan.

```sh
cargo fmt --check
cargo clippy --all-targets -- -D warnings
cargo test
```

Tambahkan tes terfokus untuk aturan baru yang rawan regresi. Untuk perubahan drag-and-drop, clipboard, pengaturan jendela, atau fokus, uji juga di sesi Wayland nyata dan catat compositor, file manager, serta hasil. Gunakan berkas uji sementara untuk operasi integrasi. Nyatakan pemeriksaan yang belum bisa dijalankan.
