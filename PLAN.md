# Rencana Implementasi Shelfdrop

Repo aplikasi berawal dari kosong. Gunakan Rust sebagai bahasa utama dan Cargo untuk build serta test. GTK4 melalui gtk-rs menjadi pilihan awal untuk UI desktop, GDK untuk drag-and-drop dan clipboard, serta GIO untuk akses metadata dan pembukaan berkas. Gunakan GTK 4.8 sebagai baseline awal karena konstruktor GdkFileList tersedia mulai versi tersebut. Validasi terhadap distribusi target saat tahap pertama. [GdkFileList](https://docs.gtk.org/gdk4/struct.FileList.html)

## 1. Buktikan alur Wayland

- [x] Buat aplikasi Cargo dengan satu jendela GTK4 dan satu entri aplikasi desktop. Catat cara memasang dependensi sistem dan menjalankan dari source.
- [x] Uji drop dua berkas dari file manager dan drag salin kembali ke folder tujuan. Pakai GtkDropTargetAsync, GtkDragSource, dan GdkContentProvider. Mulai dengan GdkFileList. Tambahkan format URI hanya bila klien target memerlukannya dan hasil uji menunjukkan manfaat. [Panduan DnD GTK](https://docs.gtk.org/gtk4/drag-and-drop.html) · [GdkContentProvider](https://docs.gtk.org/gdk4/class.ContentProvider.html)
- [ ] Uji sebagai jendela Wayland biasa pada GNOME dan KDE Plasma. Catat perilaku move dan resize yang dikelola compositor. Aktifkan opsi penempatan manual atau Keep above hanya melalui kemampuan yang dapat dideteksi dan diverifikasi. Gunakan System default pada lingkungan lain. [Wayland xdg-shell](https://wayland.app/protocols/xdg-shell)
- [x] Selesaikan tahap ini saat satu transfer pulang-pergi berjalan pada sesi Wayland nyata dan keterbatasan tiap lingkungan terdokumentasi.

## 2. Bangun model shelf dan aksi berkas

- [x] Buat model data Rust yang menyimpan URI lokal, nama, ekstensi atau MIME, ukuran opsional, dan urutan masuk. Gunakan satu jalur penambahan untuk semua drop. Validasi jenis input, deduplikasi URI, dan terapkan batas sebelum commit.
- [x] Ambil metadata dengan GIO secara asinkron. Jaga urutan batch dan kapasitas saat drop berdekatan, serta cegah pekerjaan tertunda menambahkan kembali item setelah Clear All.
- [x] Uji unit untuk drop campuran, Unicode dan spasi, URI berulang, item tanpa ekstensi, batas penuh atau diturunkan, item hilang, serta Remove dan Clear All.
- [x] Hubungkan Open ke aplikasi default. Gunakan FileManager1 ShowItems untuk Reveal dan buka folder induk bila fitur itu tidak tersedia. Hubungkan Copy dan drag keluar ke penyedia data berkas yang sama ketika formatnya cocok. Validasi paste dari Ctrl+C ke file manager. [FileManager1](https://wiki.freedesktop.org/www/Specifications/file-manager-interface/)

## 3. Implementasikan UI dari mockup

Acuan visual berada di `../shelfdrop-design/` relatif terhadap root repo aplikasi. Bandingkan hasil implementasi dengan desain FigCraft dan pratinjau PNG. [Folder desain](../shelfdrop-design/) · [FigCraft](../shelfdrop-design/shelfdrop.figcraft) · [Pratinjau](../shelfdrop-design/shelfdrop-preview.png)

- [x] Bangun header, area isi, footer, menu konteks, dan feedback penambahan. Gunakan satu model untuk list dan grid, serta pertahankan pilihan dan fokus saat layout beralih bila item masih ada.
- [x] Terapkan keadaan kosong dengan ikon berkas solid, feedback drag masuk, dan pratinjau drag keluar. Pastikan perubahan warna dan hitungan disampaikan lewat status yang dapat diakses. Hormati preferensi reduced motion.
- [x] Terapkan pengaturan tema, posisi dan perilaku jendela, batas item, clear on exit, mode, layout, dimensi, dan opasitas. Gunakan token desain untuk warna, radius, dan ukuran yang berulang. Gunakan ikon tema desktop untuk tipe berkas dan aksi yang tersedia.
- [x] Simpan preferensi secara lokal. Simpan referensi shelf secara atomik saat pemulihan diaktifkan. Saat clear on exit aktif, model berjalan di memori dan state shelf lama dibuang. Uji baca state rusak dan entri yang sudah hilang.

## 4. Verifikasi dan siapkan rilis desktop

- [x] Jalankan `cargo fmt --check`, `cargo clippy --all-targets -- -D warnings`, dan `cargo test`.
- [ ] Uji drag masuk dan keluar, clipboard, menu konteks, kegagalan metadata, fokus keyboard, skala 100% dan 200%, viewport sempit, serta Dark dan Light. Jalankan setidaknya pada GNOME/Wayland dengan Nautilus dan Plasma/Wayland dengan Dolphin. Catat versi, skenario, hasil, dan fallback yang terlihat.
- [x] Verifikasi Clear All dan pengaturan clear on exit dengan berkas uji asli yang masih ada di disk. Uji aktivasi ulang jendela dan ketahanan saat drag dibatalkan.
- [x] Siapkan ikon aplikasi A1, entri `.desktop`, dan instruksi instalasi untuk metode distribusi yang dipilih. Validasi ikon dan sumber aset yang disertakan.
- [ ] Tutup pekerjaan implementasi setelah alur dan kriteria penerimaan di [PRD](PRD.md) terpenuhi dengan bukti test otomatis dan uji Wayland nyata.

## Hasil verifikasi Wayland

Hyprland 0.57.2, Thunar 4.20.10, dan GTK 4.24.1 pada skala 100%. Kedua jendela memakai backend Wayland. Dua berkas TXT dan CSV dari folder sementara berhasil ditarik dari Thunar ke Shelfdrop, lalu disalin kembali melalui drag. Kedua salinan cocok byte demi byte dan berkas sumber tetap tersedia. Posisi dan ukuran panel juga diuji melalui kontrol floating dan resize Hyprland.

Shift+F10 membuka menu konteks, Esc mengembalikan fokus, dan Shift+panah memilih kedua berkas. Ctrl+C lalu Ctrl+V ke Thunar menghasilkan dua salinan. Thunar memerlukan format operasi salin pada clipboard. [Implementasi clipboard Thunar](https://github.com/xfce-mirror/thunar/blob/master/thunar/thunar-clipboard-manager.c)

Enter meneruskan berkas ke aplikasi default GIO yang dikonfigurasi untuk pengujian. Aksi Reveal memanggil FileManager1.ShowItems dengan URI berkas yang dipilih dan membuka folder sumber di Thunar.

Tema Light dan Dark, Compact List, Expanded Grid, dan penyimpanan ukuran preset diuji dari Settings. Hyprland mempertahankan ukuran yang diberikan compositor ketika ukuran diminta dari Settings pada jendela terbuka. Layout berubah langsung dan ukuran yang diminta tersimpan. Delete, Clear All, dan Clear shelf on exit melepas referensi tanpa mengubah dua berkas sumber.

Tes otomatis mencakup URI lokal, penolakan folder dan URI jarak jauh, Unicode, berkas tanpa ekstensi, duplikat, kapasitas, berkas hilang, state rusak, dan penyimpanan referensi. GNOME dengan Nautilus, KDE Plasma dengan Dolphin, skala 200%, dan pembaca layar masih perlu diuji. Pemeriksaan keyboard dilakukan saat bus AT-SPI pada sesi uji tidak tersedia.

Impor gambar diuji pada Hyprland 0.57.2 dengan Chrome 155 dan Firefox 160 memakai backend Wayland. Drop PNG dari kedua browser menghasilkan thumbnail dan berkas lokal di cache. Sumber GTK Wayland yang hanya menawarkan `text/uri-list` berhasil mengimpor gambar dari server HTTP uji. PNG dari Firefox berhasil diseret dari Shelfdrop ke Thunar 4.20.10. Salinan tujuan cocok byte demi byte dan berkas cache tetap tersedia.
