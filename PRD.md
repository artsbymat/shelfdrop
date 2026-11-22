# PRD Shelfdrop

## Produk dan pengguna

Shelfdrop adalah panel mengambang untuk pengguna desktop Linux Wayland yang sering memindahkan berkas antarfolder atau aplikasi. Pengguna dapat mengumpulkan referensi berkas sementara, lalu menariknya ke tujuan tanpa harus kembali mencari berkas di file manager.

Target rilis pertama adalah aplikasi desktop lokal. Akun, sinkronisasi, ekstensi browser, pengelola clipboard umum, dan pemindahan fisik berkas oleh Shelfdrop berada di luar cakupan rilis ini.

## Alur utama

1. Buka Shelfdrop dan seret satu atau beberapa berkas lokal dari file manager, atau seret gambar atau teks dari aplikasi lain.
2. Saat drag aktif, area isi menampilkan “Drop files or text to add”. Setelah drop diterima, daftar menampilkan nama, ekstensi, ukuran, dan ikon sesuai jenis berkas. Badge header menghitung seluruh item di shelf. Baris yang baru masuk disorot singkat.
3. Pilih satu atau beberapa item, lalu seret ke aplikasi atau folder yang menerima berkas. Pratinjau menampilkan nama berkas utama dan jumlah item tambahan bila ada.
4. Gunakan tombol hapus pada item atau “Clear All” untuk melepas referensi. Berkas asli tetap di tempatnya. Shelf menawarkan transfer salin pada rilis pertama, dan tetap menyimpan referensinya setelah drag keluar.

## Persyaratan rilis pertama

| Area | Perilaku yang harus tersedia |
| --- | --- |
| Panel | Header berisi logo A1 Sela Lembut, judul, jumlah item, dan akses Settings. Area isi dapat digulir dengan header dan footer tetap terlihat. Footer menyediakan Clear All dan urutan Recently added. |
| Tampilan kosong dan drag | Tampilkan “Your shelf is empty.” dan “Drag files here to keep them within reach.” Nonaktifkan Clear All saat kosong. Saat ada drag berkas yang diterima, sorot area isi dan tampilkan pesan drop. Pulihkan tampilan semula saat drag keluar atau batal. |
| Daftar dan grid | Terima berkas lokal biasa dengan jenis campuran dalam satu drop. Tampilkan ikon atau thumbnail, nama, ekstensi bila ada, ukuran bila tersedia, dan aksi hapus yang terjangkau. Nama panjang dapat disingkat secara visual, dengan nama utuh untuk pembaca layar dan saat pengguna meminta detail. |
| Menu konteks | Open membuka dengan aplikasi default. Reveal in File Manager menyorot berkas bila tersedia, atau membuka folder induk sebagai fallback. Copy menyalin referensi berkas atau isi teks untuk ditempel di tujuan yang kompatibel. Remove from Shelf melepas referensi. |
| Pengaturan | Sediakan Theme System, Dark, dan Light. Sediakan posisi shelf, perilaku jendela termasuk Keep above, batas item, dan Clear shelf on exit. Terapkan perubahan langsung. Untuk fitur posisi atau stacking yang tidak didukung compositor, tampilkan status dan gunakan perilaku bawaan compositor. |
| Shelf Appearance | Sediakan preset Compact dan Expanded, layout List dan Grid, dimensi jendela yang dapat diubah, dan opasitas. Grid menyesuaikan jumlah kolom tanpa overflow. Kontrol, teks, dan indikator fokus tetap terbaca saat opasitas diturunkan. |
| Input dan aksesibilitas | Tab berpindah antarkontrol. Tombol panah menavigasi item. Shift+panah memperluas pilihan. Enter membuka berkas, Delete melepas item terpilih, Shift+F10 membuka menu konteks, Ctrl+C menyalin pilihan, dan Esc menutup menu atau pengaturan serta mengembalikan fokus. Klik kanan juga membuka menu konteks. Pengaturan reduced motion berlaku pada transisi. |

## Aturan data

- Shelf menyimpan URI berkas lokal dan metadata, serta isi teks biasa yang ditambahkan. Penghapusan, Clear All, dan penutupan aplikasi tidak menghapus isi disk.
- URI lokal yang sudah terdaftar tidak menambah slot kedua. Satu drop menerima item baru hingga batas yang tersedia. Ringkasan menjelaskan item yang dilewati karena duplikat, tidak tersedia, di luar cakupan, atau batas tercapai.
- Jika batas diturunkan di bawah jumlah saat ini, item yang ada tetap tersimpan. Penambahan dilanjutkan saat tersedia slot. Apabila berkas dipindah atau hilang, tampilkan status yang jelas dan biarkan pengguna melepas referensinya.
- Saat Clear shelf on exit dimatikan, pulihkan referensi antar sesi. Saat diaktifkan, jalankan shelf dalam memori dan buang state shelf yang sebelumnya tersimpan. Preferensi pengguna tetap tersimpan.
- Drop menerima berkas lokal biasa, teks UTF-8, dan gambar browser. Folder dan berkas jarak jauh umum berada di luar cakupan. Drop gambar browser menerima data raster langsung atau satu URL gambar yang dapat diunduh, lalu menyimpan salinan di cache lokal. Berkas lokal valid dalam drop yang sama tetap diproses. Berkas tanpa ekstensi tetap dapat ditambahkan.

## Default awal yang diusulkan

| Pengaturan | Default |
| --- | --- |
| Tema | Dark |
| Posisi dan perilaku | System default |
| Layout dan ukuran awal | List, 412 × 322 px logis |
| Preset | Compact 310 × 238, Expanded 656 × 360 px logis |
| Batas | 24 item |
| Opasitas | 100% |
| Clear shelf on exit | Aktif |

## Kriteria penerimaan

- Dalam sesi Wayland, pengguna dapat drop beberapa jenis berkas sekaligus dari file manager, lalu menyeret satu atau beberapa item ke folder atau aplikasi kompatibel. Menggagalkan drag tidak mengubah daftar maupun isi disk.
- Teks biasa dapat diseret masuk dari aplikasi lain dan keluar kembali sebagai teks UTF-8 dengan spasi dan baris baru yang utuh.
- Gambar yang diseret dari browser muncul sebagai thumbnail. Drag keluar membuat salinan yang dapat dibuka di folder tujuan.
- Nama dengan spasi dan Unicode, berkas tanpa ekstensi, duplikat, item yang hilang setelah ditambahkan, dan batch yang melebihi batas ditangani tanpa crash.
- Open, Reveal, Copy, Remove, dan Clear All berhasil melalui kontrol yang relevan. Kesalahan saat membuka atau menyalin menyebut tindakan yang gagal dan memberi pengguna kesempatan mencoba lagi.
- Pengguna dapat menavigasi shelf, menu konteks, dan Settings dengan keyboard. Posisi fokus terlihat pada tema gelap dan terang.
- Compact dan Expanded tetap bisa dipakai pada layar kecil, monitor dengan skala berbeda, dan compositor tanpa pengaturan posisi paksa. Pengguna tetap bisa memindahkan serta mengubah ukuran jendela sejauh compositor mengizinkan.
- Preferensi dan perilaku pemulihan shelf sesuai dengan Clear shelf on exit setelah aplikasi dibuka ulang.

## Acuan visual

Gunakan folder `../shelfdrop-design/` dari root repo aplikasi sebagai acuan UI. Acuan mencakup empty, populated, drag-in, drag-out, context menu, Settings, Compact, dan Expanded. Pertahankan logo A1, ilustrasi rak tanpa ubin dekoratif, dan ikon berkas empty state berisi solid. Desain menunjukkan tema gelap. Tema terang mengikuti hierarki, kontras, dan affordance yang sama. [Folder desain](../shelfdrop-design/) · [FigCraft](../shelfdrop-design/shelfdrop.figcraft) · [Pratinjau](../shelfdrop-design/shelfdrop-preview.png)
