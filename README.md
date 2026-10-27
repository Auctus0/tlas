# Tlas

League of Legends için **draft scout + post-game analiz** masaüstü uygulaması.

- **Stack:** Tauri 2 · Rust · Svelte 5 (SvelteKit) · TypeScript · SQLite
- **Platform:** Windows 10/11
- **Güncelleme:** GitHub Releases + Tauri updater (imzalı)
- **Ban-safe:** Memory okuma, DLL injection, input otomasyonu, gizli bilgi gösterimi yok.

> Tlas isn't endorsed by Riot Games and doesn't reflect the views or opinions of Riot Games or anyone officially involved in producing or managing Riot Games properties. Riot Games and all associated properties are trademarks or registered trademarks of Riot Games, Inc.

---

## Özellikler (v0.1.0)

- League Client API (LCU) üzerinden otomatik bağlantı ve oyun fazı takibi
- Champ select'te takım listesi, rank/LP, son maç galibiyet oranı
- Rakip isimleri gizliyse **açığa çıkarmaz**
- Riot Web API ile son 20 maç özeti (KDA, CS, süre, queue)
- SQLite WAL cache + rate limit korumalı Riot istemcisi
- Tray'de çalışma, kapatınca arka plana küçülme, tek örnek kilidi
- GitHub Releases üzerinden imzalı otomatik güncelleme (4 saatte bir sessiz kontrol)
- Oyun ve champ select sırasında güncelleme kurulumunu engelleyen faz koruması

> Not: Rakip oyuncuların rankını göstermek için Riot API anahtarı gerekir. Anahtar yokken LCU tarafı (kendi takımın, faz, maç geçmişi) çalışmaya devam eder.

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

## Riot API anahtarı

1. https://developer.riotgames.com adresinden giriş yap.
2. Development key al (24 saatte bir yenilenir).
3. Tlas → Ayarlar → API anahtarı alanına yapıştır, platformu seç, kaydet.

Public dağıtım için **production key** gerekir. Başvuru, çalışan bir prototip ister ve haftalar–aylar sürebilir.

Güvenlik notu: v0.1.0 anahtarı `%APPDATA%/com.tlas.desktop/settings.json` içinde saklar. Yayın öncesinde Windows Credential Manager / DPAPI'ye taşınmalı.

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

Repo henüz oluşturulmadı. GitHub CLI ile:

```powershell
git init
git add .
git commit -m "feat: Tlas v0.1.0"
gh repo create Auctus0/tlas --public --source . --remote origin --push
```

Ardından yukarıdaki iki secret'ı ekle, sürümü bump'la ve tag'i push et:

```powershell
git tag v0.1.0
git push origin main --tags
```

Actions bitince draft release'i kontrol et ve **Publish** et. İlk kurulum dosyası
`Tlas_0.1.0_x64-setup.exe` olur.

---

## Ban-safe tasarım sınırları

| Yapılan | Yapılmayan |
|---|---|
| LCU HTTPS okuma (`lockfile` + basic auth) | `ReadProcessMemory` / `WriteProcessMemory` |
| Live Client Data API okuma | DLL injection / hooking |
| Riot Web API | `SendInput` / input otomasyonu |
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
│       ├── lcu.rs             # lockfile + LCU HTTP
│       ├── riot.rs            # rate limit + Riot API
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
- [x] M2 — Son maçlar listesi
- [ ] M2.1 — Maç detayı + timeline metrikleri
- [ ] M3 — Overlay penceresi, keychain, installer cilası
- [ ] M4 — Rate limiter sertleştirme, testler, Developer Portal kaydı