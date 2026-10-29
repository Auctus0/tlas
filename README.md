# Tlas

League of Legends için **draft scout + post-game analiz** masaüstü uygulaması.

- **Stack:** Tauri 2 · Rust · Svelte 5 (SvelteKit) · TypeScript · SQLite
- **Platform:** Windows 10/11
- **Güncelleme:** GitHub Releases + Tauri updater (imzalı)
- **Kurulum:** API anahtarı gerekmez — veri LCU + Data Dragon'dan gelir
- **Ban-safe:** Memory okuma, DLL injection, input otomasyonu, gizli bilgi gösterimi yok.

> Tlas isn't endorsed by Riot Games and doesn't reflect the views or opinions of Riot Games or anyone officially involved in producing or managing Riot Games properties. Riot Games and all associated properties are trademarks or registered trademarks of Riot Games, Inc.

---

## Özellikler (v0.1.1)

- League Client API (LCU) üzerinden otomatik bağlantı ve oyun fazı takibi
- **Genel** sekmesinde hesap özeti: Riot ID, seviye, solo/flex derece ve LP
- LP geçmişi, son 20 maç formu, şampiyon bazlı galibiyet oranı ve KDA
- **Canlı lobi takibi**: lobideki oyuncular, roller ve dereceleri
- Champ select'te takım listesi, rank/LP, son maç galibiyet oranı
- Rakip isimleri gizliyse **açığa çıkarmaz**
- Son 20 maç özeti: şampiyon, KDA, CS, süre, queue ve rol
- Şampiyon adları ve görselleri Data Dragon'dan (Türkçe yerelleştirme)
- SQLite WAL cache: scout verisi 10 dakika, kalanı oturum içi
- Tray'de çalışma, kapatınca arka plana küçülme, tek örnek kilidi
- GitHub Releases üzerinden imzalı otomatik güncelleme (4 saatte bir sessiz kontrol)
- Oyun ve champ select sırasında güncelleme kurulumunu engelleyen faz koruması

> API anahtarı, kayıt veya Riot Developer Portal hesabı gerekmez. Tlas yalnızca
> açık League Client'ın yerel API'sini (LCU) ve herkese açık Data Dragon
> dosyalarını okur. League kapalıyken son önbellek verisi gösterilir.

---

## Geliştirme

Gereksinimler:

- Node.js 22+ (24 ile test edildi)
- Rust 1.80+ (`rustup` ile)
- WebView2 (Windows 10/11'de genelde hazır)
- Visual Studio Build Tools (C++ workload)

```bash
npm install
npm run tauri dev
```

Üretim derlemesi (updater artefaktı imzalamak için anahtar gerekir):

```powershell
$env:TAURI_SIGNING_PRIVATE_KEY = Get-Content "$HOME\.tauri\tlas.key" -Raw
$env:TAURI_SIGNING_PRIVATE_KEY_PASSWORD = Get-Content "$HOME\.tauri\tlas.key.pw" -Raw
npm run tauri build
```

Çıktı: `src-tauri/target/release/bundle/nsis/`.

Faydalı komutlar:

```bash
npm run check          # Svelte + TypeScript kontrolü
npm run build          # Frontend build
cargo check            # src-tauri içinde
cargo test             # Rust birim testleri
```

---

## Veri kaynağı

Tlas hiçbir API anahtarı istemez. İki kaynak kullanılır:

| Kaynak | Ne için | Nereden |
|---|---|---|
| LCU (`https://127.0.0.1:<port>`) | Hesap, faz, champ select, rank/LP, maç geçmişi | League Client'ın `lockfile` dosyası (basic auth) |
| Data Dragon | Şampiyon adları, ikonları, sürüm | `ddragon.leagueoflegends.com` (herkese açık, CDN) |

LCU bağlantısı `lockfile` üzerinden kurulur. Farklı bir sürücüye kurduysan
`LEAGUE_LOCKFILE` ortam değişkeniyle yolu belirtebilirsin:

```powershell
$env:LEAGUE_LOCKFILE = "F:\Riot Games\League of Legends\lockfile"
```

Rank verisi `/lol-ranked/v1/ranked-stats/{puuid}` üzerinden okunur; solo queue
yoksa flex queue'ya düşer. Maç rolleri özet listede boş geldiği için
`/lol-match-history/v1/games/{gameId}` detayından tamamlanır.

> Rakip isimleri gizliyse (dereceli champ select) Tlas bunları çözmez; kartlar
> "Gizli oyuncu" olarak kalır. Bu bilinçli bir ban-safe sınırıdır.

---

## Güncelleme yayınlama

İlk kurulumdan sonra kullanıcılar Tlas içinden güncelleyebilir.

### 1. İmza anahtarı

Bu makinede anahtar üretildi:

- Private key: `%USERPROFILE%\.tauri\tlas.key`
- Şifre: `%USERPROFILE%\.tauri\tlas.key.pw`
- Public key: `src-tauri/tauri.conf.json` → `plugins.updater.pubkey` (zaten ekli)

Private key'i ve şifreyi **asla** repo'ya ekleme. Yeni bir makinede tekrar üretmek gerekirse:

```bash
npx tauri signer generate -w "$HOME/.tauri/tlas.key"
```

### 2. GitHub Secrets

Repo → Settings → Secrets and variables → Actions:

| Secret | Değer |
|---|---|
| `TAURI_SIGNING_PRIVATE_KEY` | `tlas.key` dosyasının içeriği |
| `TAURI_SIGNING_PRIVATE_KEY_PASSWORD` | `tlas.key.pw` dosyasındaki şifre |

`gh` ile:

```powershell
gh secret set TAURI_SIGNING_PRIVATE_KEY --repo Auctus0/tlas --body (Get-Content "$HOME\.tauri\tlas.key" -Raw)
gh secret set TAURI_SIGNING_PRIVATE_KEY_PASSWORD --repo Auctus0/tlas --body (Get-Content "$HOME\.tauri\tlas.key.pw" -Raw)
```

### 3. Endpoint

`src-tauri/tauri.conf.json` içinde güncelleme adresi:

```json
"endpoints": [
  "https://github.com/Auctus0/tlas/releases/latest/download/latest.json"
]
```

Repo public olmalı; değilse ayrı bir public `releases` repo'su kullan.

### 4. Sürüm bump + tag

Şu üç dosyada sürümü aynı yap:

- `package.json`
- `src-tauri/Cargo.toml`
- `src-tauri/tauri.conf.json`

```bash
git tag v0.1.1
git push origin main --tags
```

`.github/workflows/release.yml` Windows build alır, imzalar, draft release + `latest.json` üretir. GitHub'da release'i **Publish** et; eski istemciler bir sonraki kontrolde güncellemeyi görür.

Detaylar: [`UPDATER.md`](UPDATER.md)

---

## GitHub'a ilk yayın

Repo: `Auctus0/tlas` (public). Sıfırdan kurmak gerekirse:

```powershell
git init -b main
git add .
git commit -m "feat: Tlas v0.1.1"
gh repo create Auctus0/tlas --public --source . --remote origin --push
```

Ardından yukarıdaki iki secret'ı ekle, sürümü bump'la ve tag'i push et:

```powershell
git tag v0.1.1
git push origin main --tags
```

Actions bitince draft release'i kontrol et ve **Publish** et. İlk kurulum dosyası
`Tlas_0.1.1_x64-setup.exe` olur.

---

## Ban-safe tasarım sınırları

| Yapılan | Yapılmayan |
|---|---|
| LCU HTTPS okuma (`lockfile` + basic auth) | `ReadProcessMemory` / `WriteProcessMemory` |
| Live Client Data API okuma | DLL injection / hooking |
| Data Dragon (şampiyon adı/ikonu) | `SendInput` / input otomasyonu |
| Rune sayfası yazma (ileride, kullanıcı tıklarsa) | Auto-accept / auto-ban / auto-dodge |
| Harici tray + pencere | Oyun içine injection overlay |
| Görünen bilgiyi gösterme | Enemy cooldown / gizli bilgi / MMR-ELO |

LCU resmî olarak desteklenmiyor; Riot süreklilik garantisi vermiyor. Ürünü Developer Portal'a kaydetmek ve Kore'de LCU özelliklerini kapatmak gerekir.

---

## Proje yapısı

```
.
├── src/                       # Svelte 5 + TS arayüz
│   ├── lib/
│   │   ├── api.ts             # Tauri invoke sarmalayıcıları
│   │   ├── format.ts          # format yardımcıları
│   │   ├── types.ts
│   │   └── components/
│   └── routes/
│       ├── +layout.svelte
│       └── +page.svelte
├── src-tauri/
│   ├── capabilities/          # Tauri izinleri
│   ├── icons/                 # Tlas uygulama ikonları
│   └── src/
│       ├── lib.rs             # Tauri setup + tek örnek + tray + window
│       ├── commands.rs        # IPC yüzeyi + testler
│       ├── lcu.rs             # lockfile + LCU HTTP (rank, maç geçmişi)
│       ├── db.rs              # SQLite cache
│       ├── config.rs          # ayarlar
│       └── state.rs
├── assets/tlas-icon.png       # ikon kaynağı
├── CHANGELOG.md               # sürüm notları
├── ARCHITECTURE.md            # teknoloji + mimari + optimizasyon
├── RESEARCH.md                # ban riski araştırması
└── UPDATER.md                 # GitHub Releases + updater
```

---

## Yol haritası

- [x] M0 — Tauri iskelet, tray, LCU bağlantısı, faz takibi
- [x] M1 — Champ select kartları, rank/winrate, SQLite cache
- [x] M2 — Son maçlar listesi (LCU, API anahtarsız)
- [ ] M2.1 — Maç detayı + timeline metrikleri
- [ ] M3 — Overlay penceresi, installer cilası
- [ ] M4 — LCU WebSocket event'leri, testler, Developer Portal kaydı