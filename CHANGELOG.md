# Changelog

Tüm önemli değişiklikler bu dosyada tutulur. Sürümleme [SemVer](https://semver.org/) esas alır.

## Geliştirme sürümü

- Ayarlardan ayrı ayrı açılabilen LCU otomatik maç kabulü, pick ve ban eklendi.
- Seçilen pick/ban veya maç kabul komutu 2–3 saniye rastgele gecikmeyle gönderilir.
- Otomasyon ayarları varsayılan olarak kapalıdır.

## [0.1.3] — 2026-10-30

Şampiyon seçimi kaplaması eklendi: LoL istemcisinin solunda açılan ayrı bir
pencere, rakipleri tier ve karşı önerilerle özetler.

### Eklendi

- **Şampiyon seçimi kaplaması**: LoL istemcisinin soluna hizalanan, çerçevesiz,
  her zaman üstte ve odak çalmayan 360×400 pencere. Yalnızca şampiyon seçimi
  sırasında görünür; istemci taşınırsa yeniden hizalanır.
- Her rakip için şampiyon ikonu, pozisyon, OP.GG tier etiketi ve tahmini karşı
  rün/item önerisi. Deneysel meta kapalıyken tier yerine "—" gösterilir.
- Kaplama, ana pencere tepsiye kapalıyken de çalışır (arka plan faz izleme).

### Değişti

- Data Dragon yüklemesi ana pencere ve kaplama arasında paylaşılan
  `ddragon.ts` modülüne taşındı.

### Güvenlik sınırları

- Kaplama oyun içine injection yapmaz; ayrı bir masaüstü penceresidir.
- Karşı öneriler Data Dragon şampiyon etiketlerinden türetilen tahminlerdir,
  kesin counter verisi değildir.
- Rakip isimleri gizliyse kaplamada da açığa çıkarılmaz.

## [0.1.2] — 2026-10-30

Maç detayı, canlı maç, kişisel build ve kompozisyon analizi eklendi. Hesap
özeti artık daha az LCU isteğiyle çalışıyor.

### Eklendi

- **Maç detayı**: Maçlar sekmesinde bir satıra tıklayınca iki takımlı skorboard
  açılır. Rol, KDA, CS, altın, hasar, görüş ve item ikonları gösterilir.
- **Canlı maç paneli**: Oyun sırasında takım ve rakip şampiyonları 15 saniyede
  bir yenilenir. Rakip kimliği hiçbir zaman çözülmez; yalnızca şampiyon görünür.
- **Kişisel build**: Şampiyonlar panelinde bir şampiyona tıklayınca son 50 maçtan
  çekirdek item, ayakkabı, galibiyet oranı, KDA ve ortalama CS özeti çıkar.
- **Komp analizi**: Şampiyon seçimi sekmesinde iki takım için fiziksel/büyü
  hasarı, tank, ön saf ve menzilli dağılımı ile eksik uyarıları gösterilir.
- **Bugün paneli**: Günlük maç sayısı, galibiyet oranı, ortalama KDA, LP değişimi
  ve seri.
- **LP sparkline**: LP geçmişinin son 12 kaydı SVG çizgi olarak gösterilir.
- **OP.GG bağlantısı**: Şampiyon build panelinden OP.GG sayfası tarayıcıda
  açılabilir (`tauri-plugin-opener`).
- **Deneysel OP.GG meta**: Ayarlardan açılan isteğe bağlı modül; tier, galibiyet,
  seçilme, ban oranı, çekirdek item ve rün özeti. Varsayılan kapalı.
- Maç geçmişi 60 sn, ustalık 1 saat cache'lenir; hesap özeti 20 sn'lik
  yenilemede 50 maçlık isteği tekrarlamaz.

### Değişti

- LCU istemcisi 4 sn timeout, iki denemeli retry ve kimlik bilgisi yenileme
  kullanıyor; lockfile tüm sürücülerde aranıyor.
- SQLite şeması `PRAGMA user_version` tabanlı v1/v2 migration'a geçti.

### Güvenlik sınırları

- OP.GG meta yalnızca şampiyon adı ve pozisyon gönderir; Riot ID, PUUID veya
  hesap bilgisi gönderilmez. Yanıtlar 24 saat cihazda cache'lenir ve modül
  varsayılan olarak kapalıdır.
- Rakip isimleri gizliyse açığa çıkarılmaz; rakip tarafında yalnızca şampiyon
  gösterilir.

### Bilinen sınırlar

- Canlı maç paneli yalnızca oyun içi fazlarda (`InProgress`, `GameStart`,
  `Reconnect`) görünür.
- OP.GG meta deneysel bir MCP uç noktasına dayanır; kesinti durumunda OP.GG
  bağlantısı çalışmaya devam eder.

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

- Gizli bilgi gösterimi, MMR/ELO hesaplama ve rakip cooldown takibi yok.
- Riot API anahtarı yalnızca yerel `settings.json` içinde tutulur; v0.2'de DPAPI/Credential Manager'a taşınacak.

### Bilinen sınırlar

- Riot production API anahtarı olmadan yalnızca LCU verileri ve takım tarafı çalışır.
- Riot API anahtarı olmadan rakip/ally rank ve winrate gösterilmez.
- Maç detayı ve timeline ekranı henüz yok.
