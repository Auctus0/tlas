# Changelog

Tüm önemli değişiklikler bu dosyada tutulur. Sürümleme [SemVer](https://semver.org/) esas alır.

## [0.1.0] — 2026-10-27

İlk sürüm.

### Eklendi

- Tauri 2 + Rust + Svelte 5 (SvelteKit) + TypeScript iskeleti.
- League Client API (LCU) bağlantısı: lockfile keşfi, basic auth, gameflow faz takibi.
- Champ select scout: takım listesi, şampiyon ikonları, atanmış rol, rank/LP ve son 5 maç galibiyet oranı.
- Rakip oyuncu isimleri yalnızca LCU zaten açık ediyorsa gösterilir; gizli isimler çözülmez.
- Riot Web API istemcisi: `league-v4` ve `match-v5`, geliştirme anahtarı için güvenli rate limit.
- Son 20 maç özeti: şampiyon, KDA, CS, süre, queue ve rol.
- SQLite WAL cache: scout verisi 5 dakika, maç detayları 30 gün, maç listesi 90 saniye.
- Sınırlı eşzamanlı Riot istekleri ve kart bazında paralel zenginleştirme.
- Masaüstü yoğunluğunda koyu arayüz: düz paneller, Segoe UI, neon/glow ve dekoratif ikon yok; Genel, Champ Select, Maçlar, Ayarlar sekmeleri ve alt durum çubuğu.
- Tray-first davranış: pencere kapanınca tepsiye küçülür, tepsi menüsünden çıkış.
- Tek örnek kilidi: ikinci açılışta mevcut pencere öne gelir.
- GitHub Releases + Tauri updater: imzalı güncelleme, manuel kontrol ve 4 saatlik arka plan kontrolü.
- Oyun/champ select sırasında güncelleme kontrolü ve kurulumu engelleyen faz koruması.
- NSIS `currentUser` kurulumu, özel Tlas ikonu ve CSP.
- GitHub Actions Windows release workflow'u.

### Güvenlik sınırları

- Memory okuma, DLL injection, input otomasyonu, auto-accept/ban/pick/dodge yok.
- Gizli bilgi gösterimi, MMR/ELO hesaplama ve rakip cooldown takibi yok.
- Riot API anahtarı yalnızca yerel `settings.json` içinde tutulur; v0.2'de DPAPI/Credential Manager'a taşınacak.

### Bilinen sınırlar

- Riot production API anahtarı olmadan yalnızca LCU verileri ve takım tarafı çalışır.
- Riot API anahtarı olmadan rakip/ally rank ve winrate gösterilmez.
- Maç detayı ve timeline ekranı henüz yok.
