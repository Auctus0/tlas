# Tlas

LoL için taslak (draft) ve maç sonrası analiz masaüstü uygulaması.

Tlas, LoL istemcisine salt okunur bağlanır; hesap özetini, şampiyon seçimini ve
maç geçmişini tek ekranda toplar. İsteğe bağlı olarak şampiyon seçimi sırasında
istemcinin yanında küçük bir rakip kaplaması açar.

- **Stack:** Tauri 2 · Rust · Svelte 5 (SvelteKit) · TypeScript · SQLite
- **Platform:** Windows 10/11
- **Veri:** Riot API anahtarı gerekmez — LoL istemcisi (LCU) + Data Dragon

> Tlas isn't endorsed by Riot Games and doesn't reflect the views or opinions of Riot Games or anyone officially involved in producing or managing Riot Games properties. Riot Games and all associated properties are trademarks or registered trademarks of Riot Games, Inc.

---

## Özellikler

- LoL istemcisine otomatik bağlantı ve oyun fazı takibi
- **Genel:** hesap özeti, solo/flex derece, LP geçmişi ve sparkline
- **Bugün:** günlük maç sayısı, galibiyet oranı, KDA, LP değişimi ve seri
- Son 20 maç formu, şampiyon bazlı galibiyet oranı ve ustalık puanı
- **Canlı lobi:** lobideki oyuncular, roller ve dereceleri
- **Canlı maç:** oyun sırasında takım ve rakip şampiyonları
- **Şampiyon seçimi:** takım/rakip listesi, rank/LP ve son maç galibiyet oranı
- **Rakip kaplaması:** şampiyon seçimi sırasında istemcinin solunda açılan,
  her rakip için tier ve tahmini karşı rün/item gösteren ayrı pencere
- **Komp analizi:** fiziksel/büyü hasarı, tank, ön saf ve menzilli dağılımı
- **Maç detayı:** iki takımlı skorboard, item ikonları, altın/hasar/görüş
- **Kişisel build:** son 50 maçtan çekirdek item, ayakkabı, KDA ve ortalama CS
- **OP.GG bağlantısı:** şampiyon build sayfasını tarayıcıda açar
- İsteğe bağlı **deneysel OP.GG meta** modülü (varsayılan kapalı)
- Sistem tepsisinde çalışma ve tek örnek kilidi

Rakip isimleri gizliyse Tlas bunları açığa çıkarmaz.

---

## Kurulum

[Releases](https://github.com/Auctus0/tlas/releases/latest) sayfasından
`Tlas_x64-setup.exe` dosyasını indirip çalıştır. Kurulum yönetici yetkisi istemez.

Tlas'ı açtıktan sonra LoL istemcisini başlatman yeterli. Uygulama tepside
çalışmaya devam eder; pencereyi kapatmak uygulamayı kapatmaz.

---

## Kullanım

- **Genel:** hesap, derece ve son maç özeti.
- **Şampiyon Seçimi:** takım ve rakip kartları, komp analizi ve kişisel build.
  Rakip kaplaması bu sekme açık olmasa da şampiyon seçimi başladığında görünür.
- **Maçlar:** maç listesine tıklayınca skorboard açılır.
- **Ayarlar:** deneysel meta, güncelleme kontrolü ve isteğe bağlı LCU otomasyonu.

---

## Veri kaynağı

| Kaynak | Ne için | Nereden |
|---|---|---|
| LCU (`https://127.0.0.1:<port>`) | Hesap, faz, şampiyon seçimi, canlı maç, rank/LP, maç geçmişi ve detayı | LoL istemcisi `lockfile` (basic auth) |
| Data Dragon | Şampiyon/item adları, ikonlar, sürüm | `ddragon.leagueoflegends.com` (herkese açık CDN) |
| OP.GG MCP (isteğe bağlı) | Deneysel şampiyon meta özeti | Varsayılan kapalı, 24 saat cache |

LCU bağlantısı `lockfile` üzerinden kurulur. Farklı bir sürücüye kurduysan
`LEAGUE_LOCKFILE` ortam değişkeniyle yolu belirtebilirsin:

```powershell
$env:LEAGUE_LOCKFILE = "F:\Riot Games\League of Legends\lockfile"
```

OP.GG modülü açıkken bile yalnızca şampiyon adı ve pozisyon gönderilir; Riot ID,
PUUID veya hesap bilgisi gönderilmez.

---

## Sınırlar

| Tlas ne yapar | Tlas ne yapmaz |
|---|---|
| LCU HTTPS okuma (`lockfile` + basic auth) | Bellek okuma veya yazma |
| Live Client Data API okuma | DLL injection / hooking |
| Data Dragon'dan şampiyon/item verisi | `SendInput` ile tuş/fare otomasyonu |
| Ayrı, always-on-top kaplama penceresi | Oyun içine injection overlay |
| Yalnızca görünen bilgiyi gösterme | Enemy cooldown / gizli bilgi / MMR-ELO |
| Ayarlardan açılan isteğe bağlı LCU komutları | Varsayılan olarak açık otomasyon |

LCU resmî olarak desteklenmiyor; Riot süreklilik garantisi vermiyor. İsteğe bağlı
LCU otomasyonu (maç kabulü, pick, ban) varsayılan olarak kapalıdır, 2–3 saniye
rastgele gecikmeyle çalışır ve istemci sürümleriyle değişebilecek yazma uç
noktalarına dayanır.

Hiçbir kimlik bilgisi toplanmaz veya saklanmaz. LP geçmişi yalnızca cihazda
tutulur ve `clear_cache` ile silinmez; kullanıcı verisi sayılır.

---

## Geliştirme

Gereksinimler:

- Node.js 22+
- Rust 1.80+ (`rustup` ile)
- WebView2 (Windows 10/11'de genelde hazır)
- Visual Studio Build Tools (C++ workload)

```bash
npm install
npm run tauri dev
```

Faydalı komutlar:

```bash
npm run check          # Svelte + TypeScript kontrolü
npm run build          # Frontend build
cargo check            # Rust kontrolü
cargo test             # Rust birim testleri
```

Rust komutları `src-tauri` dizininde çalıştırılır.

---

## Proje yapısı

```
.
├── src/                       # Svelte 5 + TS arayüz
│   ├── lib/
│   │   ├── api.ts             # Tauri invoke sarmalayıcıları
│   │   ├── format.ts          # format yardımcıları
│   │   ├── composition.ts     # Data Dragon ile komp analizi
│   │   ├── counter.ts         # tahmini karşı rün/item
│   │   ├── ddragon.ts         # paylaşılan Data Dragon yükleyici
│   │   ├── types.ts
│   │   └── components/        # PlayerCard, MatchRow, MatchDetail, DraftComp, Sparkline
│   └── routes/
│       ├── +layout.svelte
│       ├── +page.svelte       # ana pencere
│       └── overlay/+page.svelte
├── src-tauri/
│   ├── capabilities/          # Tauri izinleri
│   ├── icons/                 # Tlas uygulama ikonları
│   └── src/
│       ├── lib.rs             # Tauri setup + tek örnek + tray
│       ├── overlay.rs         # kaplama hizalama ve faz izleme
│       ├── commands.rs        # IPC yüzeyi + testler
│       ├── lcu.rs             # lockfile + LCU HTTP
│       ├── opgg.rs            # deneysel OP.GG MCP istemcisi
│       ├── db.rs              # SQLite cache + migration
│       ├── config.rs          # ayarlar
│       └── state.rs
├── assets/tlas-icon.png
├── CHANGELOG.md
└── ARCHITECTURE.md
```
