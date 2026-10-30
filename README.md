# Tlas

League of Legends için **draft scout + maç sonrası analiz** masaüstü uygulaması.

- **Stack:** Tauri 2 · Rust · Svelte 5 (SvelteKit) · TypeScript · SQLite
- **Platform:** Windows 10/11
- **Veri:** Riot API anahtarı gerekmez — LCU + Data Dragon
- **Ban-safe:** Memory okuma, DLL injection, input otomasyonu ve gizli bilgi gösterimi yok

> Tlas isn't endorsed by Riot Games and doesn't reflect the views or opinions of Riot Games or anyone officially involved in producing or managing Riot Games properties. Riot Games and all associated properties are trademarks or registered trademarks of Riot Games, Inc.

---

## Özellikler

- League Client API (LCU) üzerinden otomatik bağlantı ve oyun fazı takibi
- **Genel**: hesap özeti, solo/flex derece, LP geçmişi ve SVG sparkline
- **Bugün**: günlük maç sayısı, galibiyet oranı, KDA, LP değişimi ve seri
- Son 20 maç formu, şampiyon bazlı galibiyet oranı ve ustalık puanı
- **Canlı lobi**: lobideki oyuncular, roller ve dereceleri
- **Canlı maç**: oyun sırasında takım ve rakip şampiyonları
- **Şampiyon seçimi**: takım/rakip listesi, rank/LP ve son maç galibiyet oranı
- **Komp analizi**: fiziksel/büyü hasarı, tank, ön saf ve menzilli dağılımı
- **Maç detayı**: iki takımlı skorboard, item ikonları, altın/hasar/görüş
- **Kişisel build**: son 50 maçtan çekirdek item, ayakkabı, KDA ve ortalama CS
- **OP.GG bağlantısı**: şampiyon build sayfasını tarayıcıda açar
- İsteğe bağlı **deneysel OP.GG meta** modülü (varsayılan kapalı)
- Tray'de çalışma, tek örnek kilidi ve imzalı otomatik güncelleme
- Oyun ve şampiyon seçimi sırasında güncelleme kurulumunu engelleyen faz koruması

Rakip isimleri gizliyse Tlas bunları açığa çıkarmaz.

---

## Kurulum

[Releases](https://github.com/Auctus0/tlas/releases/latest) sayfasından
`Tlas_x64-setup.exe` dosyasını indirip çalıştır. Kurulum yönetici yetkisi istemez.

Güncellemeler uygulama içinden imzalı olarak indirilir; tarayıcıya veya manuel
dosya indirmeye gerek yoktur.

---

## Veri kaynağı

| Kaynak | Ne için | Nereden |
|---|---|---|
| LCU (`https://127.0.0.1:<port>`) | Hesap, faz, şampiyon seçimi, canlı maç, rank/LP, maç geçmişi ve detayı | League Client `lockfile` (basic auth) |
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

## Gizlilik ve ban-safe sınırlar

| Yapılan | Yapılmayan |
|---|---|
| LCU HTTPS okuma (`lockfile` + basic auth) | `ReadProcessMemory` / `WriteProcessMemory` |
| Live Client Data API okuma | DLL injection / hooking |
| Data Dragon (şampiyon/item verisi) | `SendInput` / input otomasyonu |
| Harici tray + pencere | Oyun içine injection overlay |
| Görünen bilgiyi gösterme | Enemy cooldown / gizli bilgi / MMR-ELO |
| Kullanıcı tıklarsa OP.GG deep link | Otomatik kabul / ban / pick / dodge |

Hiçbir kimlik bilgisi toplanmaz veya saklanmaz. LP geçmişi yalnızca cihazda
tutulur ve `clear_cache` ile silinmez; kullanıcı verisi sayılır.

LCU resmî olarak desteklenmiyor; Riot süreklilik garantisi vermiyor.

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
cargo check            # src-tauri içinde
cargo test             # Rust birim testleri
```

---

## Proje yapısı

```
.
├── src/                       # Svelte 5 + TS arayüz
│   ├── lib/
│   │   ├── api.ts             # Tauri invoke sarmalayıcıları
│   │   ├── format.ts          # format yardımcıları
│   │   ├── composition.ts     # Data Dragon ile komp analizi
│   │   ├── types.ts
│   │   └── components/        # PlayerCard, MatchRow, MatchDetail, DraftComp, Sparkline
│   └── routes/
│       ├── +layout.svelte
│       └── +page.svelte
├── src-tauri/
│   ├── capabilities/          # Tauri izinleri
│   ├── icons/                 # Tlas uygulama ikonları
│   └── src/
│       ├── lib.rs             # Tauri setup + tek örnek + tray + window
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

---

## Planlanan

- Timeline metrikleri (CS/dk, gold diff, objective participation)
- LCU WebSocket event'leri
- Overlay penceresi ve gelişmiş taslak önerileri
