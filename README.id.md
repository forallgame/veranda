<p align="center">
  <img src="apps/desktop/src-tauri/icons/128x128.png" width="96" alt="Padu">
</p>

<h1 align="center">Padu</h1>

<p align="center">
  Server web lokal yang ringan dan bersahabat untuk proyek PHP.<br>
  Satu jendela, beberapa klik, dan <code>https://aplikasi-saya.test</code> sudah berjalan di komputer Anda.
</p>

<p align="center">
  <a href="#unduh">Unduh</a> ·
  <a href="#apa-yang-anda-dapat">Apa yang Anda dapat</a> ·
  <a href="#lima-menit-pertama">Lima menit pertama</a> ·
  <a href="README.md">English</a>
</p>

---

Padu dibuat untuk Anda yang membangun situs dengan PHP dan ingin tumpukan server lokal yang
biasa dipakai langsung jalan, tanpa menghabiskan akhir pekan untuk menyiapkannya. Anggap saja
ini alternatif yang tenang dari paket "serba ada" yang besar: cepat dibuka, tidak mengganggu,
dan hanya mengunduh bagian yang memang Anda pakai.

![Padu dalam mode gelap dan terang](docs/screenshot-dark.png)

## Apa yang Anda dapat

- **Seluruh tumpukan, Anda yang memilih.** PHP (8.1 sampai 8.5 terbaru), server web (bawaan
  Caddy; Nginx atau Apache jika lebih suka), MariaDB atau MySQL, Valkey/Redis, Mailpit untuk
  menangkap email, serta Node.js, Composer, Adminer, dan phpMyAdmin. Pilih versinya dari daftar,
  ganti kapan saja, pasang beberapa versi berdampingan.
- **Folder proyek otomatis menjadi situs.** Taruh folder di `www/`, maka ia dilayani di
  `https://nama-folder.test` dengan sertifikat yang dipercaya browser. Folder `public/` milik
  Laravel dan Symfony dikenali sendiri.
- **Jujur tentang apa yang sedang terjadi.** Setiap layanan menampilkan status, port, dan versi.
  Unduhan punya bilah kemajuan sungguhan. Ada tab Log kalau ada yang tidak beres, jadi Anda tidak
  perlu menebak.
- **Ringan di komputer.** Aplikasinya sendiri sekitar 9 MB. Tidak ada yang dibundel; runtime
  diambil dari server unduhan resminya, diverifikasi dengan checksum, hanya saat Anda meminta.
- **Aman sejak awal.** Semua layanan hanya mendengarkan di localhost. Database mendapat kata sandi
  root acak saat pertama dijalankan, bukan kosong. HTTPS memakai otoritas sertifikat lokal yang
  tidak pernah keluar dari mesin Anda. Tanpa akun, tanpa pelacakan, tanpa lalu lintas jaringan
  diam-diam.
- **Selalu segar.** Padu memberi tahu saat versi baru terbit dan memasangnya dengan satu klik
  (atau otomatis, kalau Anda menyalakannya). Daftar paket selalu tahu PHP, MariaDB, dan kawan-
  kawannya yang terbaru.
- **Sama di Linux, Windows, dan macOS.** Mode gelap dan terang tersedia.

## Unduh

Ambil pemasang untuk sistem Anda dari halaman **[Releases](../../releases/latest)**:

| Sistem | Berkas | Catatan |
|---|---|---|
| Linux (Debian, Ubuntu, Kubuntu, Mint) | `padu_x.y.z_amd64.deb` | `sudo apt install ./padu_x.y.z_amd64.deb` |
| Linux (semua) | `padu_x.y.z_amd64.AppImage` | beri izin eksekusi lalu jalankan; yang ini bisa memperbarui dirinya |
| Linux (Fedora, openSUSE) | `padu-x.y.z-1.x86_64.rpm` | `sudo dnf install ./padu-*.rpm` |
| Windows 10/11 | `Padu_x.y.z_x64-setup.exe` | pemasang biasa; WebView2 diunduh bila belum ada |
| macOS | `Padu_x.y.z_universal.dmg` | Apple Silicon dan Intel dalam satu berkas |

Peluncuran pertama membuat folder `padu` di direktori home Anda untuk proyek, pengaturan, dan
database. Tidak ada bagian lain dari sistem yang disentuh.

Lebih suka terminal? Ada alat baris perintah `padu` di rilis yang sama dengan semua fitur
aplikasinya (`padu install php`, `padu start`, `padu project create blog --template wordpress`).

## Lima menit pertama

1. Buka Padu, masuk ke **Packages**, pasang PHP, Caddy, dan MariaDB (Mailpit juga berguna).
2. Di **Settings** tekan **Trust local CA** supaya `https://*.test` menampilkan gembok di browser.
3. Kembali ke **Dashboard**, tekan **Start All**. Saat Padu meminta memperbarui berkas hosts,
   setujui; itulah yang membuat `aplikasi-saya.test` dikenali.
4. **Sites → New project**: pilih WordPress, Laravel, Symfony, templat Composer, atau URL git.
   Database dengan nama yang sama dibuatkan untuk Anda.
5. Buka `https://aplikasi-saya.test`. Selesai.

Masuk ke database: host `127.0.0.1`, pengguna `root`, kata sandi ada di **Database → Show**.

## Perlu diketahui

- Linux: port 80 dan 443 butuh izin satu kali (`padu doctor --fix-ports`), atau pakai 8080/8443
  di Settings.
- Pemasangan `.deb`/`.rpm` di Linux tidak memperbarui dirinya; aplikasi akan membuka halaman
  rilis. AppImage bisa memperbarui sendiri.
- macOS belum ditandatangani sertifikat Apple Developer, jadi peluncuran pertama memunculkan
  peringatan "pengembang tidak dikenal" (klik kanan → Open).

## Legal

Padu adalah implementasi orisinal yang dibuat dari nol. Tidak ada kode, templat, aset, atau teks
dari produk serupa lain; nama produk lain dalam dokumen ini milik pemiliknya masing-masing dan
hanya disebut untuk identifikasi. Semua dependensi Rust berlisensi permisif (lihat
[THIRD_PARTY_NOTICES.md](THIRD_PARTY_NOTICES.md)). Runtime diunduh dari server resminya atas
permintaan Anda dan tidak didistribusikan ulang di sini.

Proyek pribadi. Lihat [LICENSE](LICENSE).
