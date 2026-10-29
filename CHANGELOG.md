# Changelog

Tüm önemli değişiklikler bu dosyada tutulur. Sürümleme [SemVer](https://semver.org/) esas alır.

## [0.1.1] — 2026-10-27

Riot Web API bağımlılığı tamamen kaldırıldı. Uygulama artık hiçbir API anahtarı
istemez.

### Eklendi

- **Genel** sekmesinde hesap özeti: Riot ID, seviye, profil ikonu, solo/flex
  derece kartları (LP, G/M, galibiyet oranı).
- **LP geçmişi**: her gözlemde derece/LP değişimi cihazda saklanır ve son 12
  kayıt farklarıyla listelenir. (LCU geçmiş vermediği için kayıtlar Tlas'ın
  kendi gözlemlerinden oluşur.)
- **Canlı lobi takibi**: lobideki oyuncular, tercih ettikleri rol ve dereceleri.
- Son 20 maç formu (galibiyet/mağlubiyet şeridi), seri ve galibiyet oranı.
- Şampiyon istatistikleri: oynanan maç, galibiyet oranı, KDA ve ustalık puanı
  (LCU `/lol-champion-mastery/v1/...`).

### Değişti

- Champ select scout'u artık LCU `/lol-ranked/v1/ranked-stats/{puuid}` üzerinden
  çalışıyor: tier, division, LP, galibiyet/mağlubiyet.
- Son maç galibiyet oranı LCU maç geçmişinden hesaplanıyor; Rift dışı queue'lar
  (ARAM, Arena, TFT) sayılmıyor.
- Son 20 maç listesi LCU `/lol-match-history/v1/...` üzerinden okunuyor.
- Maç rolleri özet yanıtında boş geldiği için `/lol-match-history/v1/games/{gameId}`
  detayından tamamlanıyor (en fazla 6 eşzamanlı, yalnızca Rift kuyrukları).
- Kuyruk adları LCU `/lol-game-queues/v1/queues` üzerinden yerelleştirilmiş
  olarak alınıyor ve bir gün cache'leniyor.
- Scout cache TTL'i 5 → 10 dakika.
- Ayarlar ekranındaki API anahtarı ve platform alanları kaldırıldı; yerine canlı
  "Veri kaynağı" paneli geldi.

### Kaldırıldı

- `riot.rs` istemcisi, `Riot` state alanı, `riotApiKey`/`platform` ayarları.
- `match_cache` tablosu (maç detayları artık kalıcı cache'lenmiyor).
- Riot rate-limit / 429 mantığı.
- Genel sekmesindeki "Veri kaynağı" satırı ve tanıtım paragrafları.

### Düzeltildi

- Maç süresi milisaniye olarak gelen kayıtlar saniyeye çevriliyor.
- Şampiyon ikonu yüklenemezse satır artık boş kutu göstermiyor.
- Bağlantı yokken "Oyun fazı" satırı ham hata yerine `—` gösteriyor.
- Derece adları Türkçeleştirildi (Platin, Zümrüt, Elmas, Şampiyon...).
- Şampiyon listesi maç sayısına göre deterministik sıralanıyor; eşitlikte
  ustalık puanı ve şampiyon kimliği kullanılıyor (liste artık yenilemede
  yer değiştirmiyor).
- **Şampiyonlar** paneli sütunlu tabloya çevrildi (şampiyon, maç, galibiyet,
  KDA, ustalık); panel ve tablo aralıkları eşitlendi.
- Arayüzdeki "League" ifadeleri "LoL" olarak sadeleştirildi; durum çubuğundaki
  güvenlik notu ve tanıtım paragrafları kaldırıldı.

### Güvenlik sınırları

- Memory okuma, DLL injection, input otomasyonu, auto-accept/ban/pick/dodge yok.
- Gizli bilgi gösterimi, MMR/ELO hesaplama ve rakip cooldown takibi yok.
- Hiçbir kimlik bilgisi toplanmıyor veya saklanmıyor.

### Bilinen sınırlar

- Dereceli champ select'te rakip isimleri LCU tarafından gizlenir; Tlas bunları
  çözmez.
- Maç detayı ve timeline ekranı henüz yok.

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
