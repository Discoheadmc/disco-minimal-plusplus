<p align="center">
  <img src="assets/logo.svg" width="128" alt="Disco Minimal++ logo (placeholder)" />
</p>

# Disco Minimal++

**100% Rust ile sıfırdan yazılmış, minimal ve performans odaklı Minecraft launcher.**
A minimal, performance-first Minecraft launcher, written from scratch in 100% Rust.

[Disco Launcher](https://github.com/Discoheadmc/Disco-Launcher)'ın ruhunu — *"iyi bir launcher'ı iyi yapan şeyi koru, geri kalanını çıkar"* — çok daha hızlı bir tabana taşıyan yeni nesil proje. UI tabanı olarak [OneLauncher](https://github.com/Polyfrost/OneLauncher) ile aynı **Freya** (Polyfrost fork) motorunu, mimari ilham olarak aynı monorepo düzenini kullanır.

## Neden Rust?

| | Disco Launcher (Electron/XMCL) | Disco Minimal++ (Rust/Freya) |
|---|---|---|
| Bellek kullanımı | ~200-400 MB | hedef ~40-80 MB |
| Açılış süresi | saniyeler | hedef milisaniyeler |
| Telemetri | yok | yok (değişmez) |
| Tek binary | hayır | evet |

## Mimari

```
packages/
├── disco_common   # sabitler, veri dizinleri, ortak hata tipleri
├── disco_net      # HTTP istemcisi; SHA1 doğrulamalı, paralel indirme motoru
├── disco_auth     # çevrimdışı (Mojang-tarzı UUID) + Microsoft hesap yapısı
├── disco_mc       # Mojang piston-meta manifest, sürüm çözümleme, dosya hazırlığı, komut üretimi
├── disco_core     # instance yönetimi, ayarlar, başlatma orkestrasyonu
└── disco_app      # Freya UI (disco_minimal++ binary'si)
```

## Durum — Kilometre Taşı 0 (iskelet)

- [x] Cargo workspace altyapısı (edition 2024, Rust 1.97+)
- [x] Mojang sürüm manifesti: çekme + ayrıştırma
- [x] Sürüm çözümleme (`inheritsFrom` zinciri dahil)
- [x] Client jar + kütüphaneler + assetler için paralel, SHA1 doğrulamalı indirme
- [x] Natives çıkarma (LWJGL)
- [x] Instance modeli (dosya tabanlı, XMCL/Prism tarzı)
- [x] Çevrimdışı hesap (gerçek Mojang `OfflinePlayer:` UUID algoritması)
- [x] Freya UI: instance listesi, sürüm seçici, başlat/durum paneli
- [ ] Microsoft hesap girişi (device code flow — yapı hazır)
- [ ] Java otomatik indirme
- [ ] Tema/renk seçici (Disco Launcher'daki 8 renk geleneği)
- [ ] Mod paketi mağazası (Modrinth/CurseForge)

## Geliştirme

**Gereksinimler:** Rust 1.97+ ([rustup](https://rustup.rs/)), Windows'ta MSVC Build Tools.

```bash
# Çalıştır
cargo run -p disco_app

# Release derle
cargo build -p disco_app --release
```

## Lisans

[GPL-3.0](LICENSE). Bu proje sıfırdan yazılmıştır; OneLauncher (GPL-3.0) ve Polyfrost Freya
fork'u mimari ve UI tabanı olarak referans alınmıştır, bu yüzden proje GPL-3.0 ile
lisanslanmıştır. Disco Launcher'ın (MIT) fikir mirası ayrı olarak yaşar.
