# Tlas — Teknoloji Kararı ve Mimari

> Kapsam: **Post-game analiz + Champ Select scout**, Windows masaüstü, tek uygulama.
> Hedef: düşük kaynak tüketimi, düşük gecikme, ban-safe tasarım.

---

## 1. Karar: Tauri 2 + Rust + Svelte 5 + TypeScript + SQLite

| Aday | Installer | RAM (idle) | Soğuk açılış | UI hızı | Windows entegrasyonu | Karar |
|---|---|---|---|---|---|---|
| **Tauri 2 + Rust + Svelte 5** | **6–12 MB** | **40–80 MB** | **< 0.5 s** | Çok iyi | Mükemmel (Win32, tray, updater) | ✅ **Seçim** |
| Electron + React | 80–150 MB | 180–350 MB | 1.5–3 s | İyi | İyi | ❌ Ağır |
| C# .NET 10 + WPF | 40–70 MB (self-contained) | 70–120 MB | 0.6–1.2 s | İyi | Mükemmel | 🥈 Alternatif |
| Python + PySide6 | 40–90 MB | 80–150 MB | 1–2 s | Orta | Orta | ❌ Paketleme/dağıtım zayıf |
| Go + Wails | 10–20 MB | 50–90 MB | < 0.5 s | İyi | İyi | 🥉 Ekosistem dar |

**Neden Tauri?**
- WebView2 Windows 10/11'de hazır → Chromium'u paketlemiyorsun, installer küçük.
- Rust backend: GC yok, champ select sırasında öngörülebilir gecikme, düşük CPU.
- Svelte 5: derlenmiş, ~10–20 KB runtime, fine-grained reactivity; saniyede birkaç kez güncellenen kartlar için ideal.
- Tauri 2 plugin'leri: tray, single-instance, autostart, updater, global shortcut, log.

**Neden C# değil?** C# tamamen geçerli bir seçim; WPF Win32 interop'u çok rahat. Ama WPF Native AOT'yi tam desteklemiyor, UI geliştirme hızı web tabanlı stack'e göre düşük. Rust öğrenme eğrisi senin için sorun değilse Tauri daha iyi sonuç verir.

**Neden Electron değil?** Bu uygulama oyun sırasında tray'de sürekli açık kalacak. 200–300 MB RAM kabul edilemez.

---

## 2. Klasör Yapısı (v0.1.3)

```
LOL/
├── package.json
├── svelte.config.js
├── vite.config.js
├── tsconfig.json
├── src/                          # Svelte 5 + TS
│   ├── app.html
│   ├── app.css
│   ├── lib/
│   │   ├── api.ts                # invoke() sarmalayıcıları
│   │   ├── format.ts             # faz/queue/rol/tier/OP.GG formatları
│   │   ├── composition.ts        # Data Dragon tags/info ile komp analizi
│   │   ├── counter.ts            # rakip tags/info ile tahmini karşı öneri
│   │   ├── ddragon.ts            # paylaşılan Data Dragon yükleyici + cache
│   │   ├── types.ts              # Rust modellerinin TS karşılığı
│   │   └── components/
│   │       ├── PlayerCard.svelte
│   │       ├── MatchRow.svelte
│   │       ├── MatchDetail.svelte
│   │       ├── DraftComp.svelte
│   │       └── Sparkline.svelte
│   └── routes/
│       ├── +layout.svelte
│       ├── +page.svelte          # Genel / Champ Select / Maçlar / Ayarlar
│       └── overlay/+page.svelte  # şampiyon seçimi kaplaması
├── src-tauri/
│   ├── Cargo.toml
│   ├── build.rs
│   ├── tauri.conf.json
│   ├── capabilities/default.json
│   ├── icons/                    # Tlas ikonları
│   └── src/
│       ├── lib.rs                # setup, tek örnek, tray, pencere
│       ├── main.rs
│       ├── overlay.rs            # Win32 hizalama + görünürlük + faz izleme
│       ├── commands.rs           # #[tauri::command] IPC + testler
│       ├── models.rs             # serde structs
│       ├── lcu.rs                # lockfile keşfi + HTTP client
│       ├── opgg.rs               # deneysel OP.GG MCP istemcisi + cache
│       ├── db.rs                 # SQLite WAL cache + LP geçmişi
│       └── config.rs             # settings.json
├── assets/tlas-icon.png
├── CHANGELOG.md
└── ARCHITECTURE.md
```

> Henüz uygulanmayan modüller: LCU WebSocket event aboneliği, `live/` Live Client
> Data API, `analysis/postgame.rs` timeline metrikleri. v0.1.2 bunları LCU
> polling + maç detayı ile karşılıyor.

---

## 3. Veri Akışı

### Champ Select (kritik yol)

```
LCU phase poll (1 Hz)
        │  "ChampSelect"
        ▼
GET /lol-champ-select/v1/session
        │  5 ally + 5 enemy (summonerId / puuid)
        ▼
┌──────────────────────────────────────────────────┐
│  LCU (paralel, kart başına join_all)             │
│  /lol-ranked/v1/ranked-stats/{puuid}             │
│      → tier / division / LP / W-L                │
│  /lol-match-history/v1/products/lol/{puuid}/     │
│      matches → son 20 maç W-L (SR filtreli)      │
└──────────────────────────────────────────────────┘
        │
        ▼
SQLite kv cache (PUUID bazlı, 10 dk TTL)
        │
        ▼
Tauri invoke → Svelte kartları
```

Şampiyon adı/ikonu ayrı bir katmandır: frontend, Data Dragon
`versions.json` + `champion.json` dosyalarını 7 gün `localStorage`'da tutar.
Riot Web API anahtarı hiçbir aşamada kullanılmaz.

**Gecikme bütçesi**
| Durum | Hedef |
|---|---|
| Cache'te olan oyuncu | < 100 ms |
| Cache boş, sıcak ağ | < 1.2 s |
| Cold start + 10 oyuncu | < 2.5 s |

**Prefetch:** v0.1.1'de champ select açıldığında istekler başlar. Planlanan: lobby
oluştuğunda (`/lol-lobby/v2/lobby`) takım arkadaşlarını çözüp champ select'te sadece
eksikleri çekmek.

### Şampiyon seçimi kaplaması

```
Rust arka plan görevi (2 sn)
        │  GET /lol-gameflow/v1/gameflow-phase
        ▼
phase ∈ {ChampSelect, GameStart, InProgress, Reconnect} ?
        │ evet                          │ hayır
        ▼                               ▼
FindWindowW("RiotWindowClass")       pencereyi gizle
GetWindowRect + MonitorFromWindow
GetMonitorInfoW → çalışma alanı
        │  sol → sağ → çalışma alanı sol kenarı
        │  (çalışma alanına clamp)
        ▼
ayrı "overlay" webview penceresi
(360×400, çerçevesiz, always-on-top, focusable=false)
        │
        ▼
Champ Select poll (4 sn) → rakip listesi
        │
        ├─► OP.GG meta (yalnızca deneysel mod açıksa) → tier
        └─► Data Dragon tags/info → tahmini rün + item
```

Kaplama ana pencereye injection yapmaz. Ayrı bir Tauri penceresidir; `overlay`
etiketli route yüklenir. Ana pencere tepsiye kapalıyken de faz izleme Rust
tarafında sürdüğü için kaplama açılmaya devam eder. İstemci minimized ise
konumlandırma istemci dikdörtgenine göre değil, monitörün çalışma alanına göre
yapılır. Deneysel meta kapalıyken OP.GG'ye hiç istek gitmez; tier yerine "—"
gösterilir.

### Genel sekmesi (hesap + lobi)

```
LCU /lol-summoner/v1/current-summoner      → Riot ID, seviye, ikon
LCU /lol-ranked/v1/ranked-stats/{puuid}    → solo + flex (tier, division, LP, W/L)
LCU /lol-match-history/.../matches (50)    → son 20 form, şampiyon istatistikleri
LCU /lol-champion-mastery/v1/{puuid}/...   → ustalık puanı
LCU /lol-lobby/v2/lobby                    → lobi üyeleri + tercih edilen rol
LCU /lol-gameflow/v1/session               → canlı maç takım/rakip şampiyonları
        │
        ▼
account_overview / lobby_view / live_game (20 sn'de bir, yalnızca Genel açıkken)
        │
        ▼
SQLite kv cache: maç geçmişi 60 sn, ustalık 1 saat
SQLite lp_history: derece/LP her değiştiğinde yeni satır (son 200 kayıt)
```

LP geçmişi LCU'da bulunmadığı için Tlas kendi gözlemlerinden oluşturur:
tier/division aynı kaldığı sürece fark (`+21`, `-18`) hesaplanır, terfi/düşüş
sonrası fark gösterilmez. Bu tablo `clear_cache` ile silinmez; kullanıcı verisi
sayılır.

Hesap özeti 20 saniyede bir yenilenir; 50 maçlık geçmiş ve ustalık ayrı TTL'lere
sahip cache'ten okunur, böylece her yenilemede LCU'ya aynı ağır istek gitmez.

### Post-game

```
LCU /lol-match-history/v1/products/lol/{puuid}/matches
        │  anahtarsız, hızlı; yalnızca istenen oyuncunun katılımı döner
        ▼
Özet satırları (KDA, CS, süre, queue)
        │  rol bilgisi özette boş ("NONE")
        ▼
/lol-match-history/v1/games/{gameId}
        │  10 katılımcı + timeline.lane/role, 6 eşzamanlı
        ▼
Rol eşlemesi: TOP/MIDDLE/JUNGLE, BOTTOM+CARRY → BOTTOM,
              BOTTOM+SUPPORT → UTILITY
        │
        ├─► Skorboard: iki takım, KDA/CS/altın/hasar/görüş/item
        └─► Kişisel build: son 50 maçta aynı şampiyonun item frekansı
```

Post-game tamamen çevrimdışı çalışır: LCU + yerel cache dışında hiçbir servise
istek gitmez. `match_detail` tam kadro skorboardunu, `personal_build` ise
kullanıcının kendi maçlarından çekirdek item/ayakkabı özetini üretir. Item
adları Data Dragon `item.json` dosyasından gelir; bilinmeyen kimlikler yalnızca
kimlik olarak gösterilir.

**Silahlanma notu:** LCU'nun maç özeti uç noktası zaman zaman
`participantIdentities` içinde tek oyuncu döndürür; bu yüzden tam kadro
bilgisi gereken her şey `/lol-match-history/v1/games/{gameId}` üzerinden
tamamlanır.

### Canlı maç

```
LCU /lol-gameflow/v1/session
        │  phase ∈ {InProgress, GameStart, Reconnect}
        ▼
teamOne (takım) → summonerId ile isim çözülür
teamTwo (rakip) → isim asla çözülmez, "Rakip N"
        │
        ▼
live_game (15 sn poll, yalnızca oyun içi fazda)
```

### Deneysel OP.GG meta (varsayılan kapalı)

```
Ayarlar → experimental_meta
        │
        ▼
opgg_meta(champion, position)
        │  yalnızca şampiyon adı + pozisyon
        ▼
https://mcp-api.op.gg/mcp  (initialize → tools/call)
        │  yanıt: Python-repr metni
        ▼
parser → MetaReport → SQLite kv cache (24 saat)
```

Modül kapalıyken hiçbir dış istek yapılmaz; arayüzde yalnızca OP.GG deep link
düğmesi kalır. Riot ID, PUUID veya hesap bilgisi hiçbir koşulda gönderilmez.

---

## 4. "Full Optimizasyon" — Somut Bütçeler

| Alan | Teknik | Hedef |
|---|---|---|
| **Binary** | `lto="fat"`, `codegen-units=1`, `opt-level="s"`, `strip=true`, `panic="abort"` | 6–12 MB installer |
| **RAM** | Lazy module load, LRU cap 50 MB, timeline stream | 40–80 MB idle |
| **Açılış** | Tray-first, LCU detector only, post-game lazy | < 500 ms |
| **CPU (idle)** | Oyuna bağlı değilken 0 polling; event-driven | < 0.1 % |
| **CPU (champ select)** | Paralel fetch, cache, tek render | < 1 % burst |
| **Ağ** | HTTP/2 keep-alive, gzip, connection pool, 5 s timeout | 10 oyuncu < 25 istek |
| **LCU yükü** | Kart başına 2 istek, eşzamanlı `join_all`, 10 dk cache | champ select'te 20 istek |
| **DB** | SQLite WAL, `synchronous=NORMAL`, `mmap_size`, indeks | Sorgu < 5 ms |
| **UI** | Svelte 5 runes, sanal liste, `content-visibility:auto` | 60 fps, layout shift yok |
| **Overlay** | Ayrı şeffaf pencere, sadece değişimde render | GPU < 1 % |
| **Paket** | `pnpm`, Vite `esbuild` minify, tree-shaking | JS bundle < 80 KB |

**Ölçmeden optimize etme.** Baştan `tracing` + `tauri-plugin-log` kur; champ select cold/warm süresini ve idle RAM'i logla. Bütçeyi aşan yeri profille.

### Cargo profili

```toml
[profile.release]
lto = "fat"
codegen-units = 1
opt-level = "s"
strip = true
panic = "abort"
```

> `opt-level = "s"` boyut içindir; CPU kritik parse işlemleri için `"3"` deneyip ölç. Champ select'te ağ gecikmesi baskın olduğu için `"s"` genelde yeterli.

---

## 5. Ban-Safe Tasarım Kuralları (kod seviyesinde)

| Kural                   | Uygulama                                                           |
| ----------------------- | ------------------------------------------------------------------ |
| Memory yok              | `ReadProcessMemory`, `OpenProcess`, `WriteProcessMemory` yasak     |
| Injection yok           | DLL inject, `SetWindowsHookEx`, `CreateRemoteThread` yok           |
| Input yok               | `SendInput`, `keybd_event`, `mouse_event` yok                      |
| Sadece LCU HTTPS        | `lockfile` → `riot:<password>` basic auth, self-signed cert ignore |
| Sadece Live Client read | `127.0.0.1:2999`, GET                                              |
| Gizli bilgi yok         | enemy cooldown, enemy jungle timer, fog-of-war bilgisi yok         |
| Dış meta isteği         | Yalnızca şampiyon + pozisyon, varsayılan kapalı OP.GG MCP          |
| MMR/ELO yok             | rank/LP göster, tahmini MMR gösterme                               |
| Yazma minimum           | Kullanıcı ayarıyla maç kabul, pick, ban (LCU PATCH)                |
| Overlay injectsiz       | Ayrı always-on-top pencere veya OBS browser source                 |

**Kore:** KR platform filtresi henüz uygulanmadı; LCU özellikleri her platformda
aynı şekilde çalışır.

---

## 6. Teknik Detaylar

### LCU lockfile keşfi

```rust
// C:\Riot Games\League of Legends\lockfile
// format: name:pid:port:password:protocol
pub fn read_lockfile() -> Option<LcuCredentials> {
    let path = PathBuf::from(r"C:\Riot Games\League of Legends\lockfile");
    let raw = std::fs::read_to_string(path).ok()?;
    let mut it = raw.trim().split(':');
    let _name = it.next()?;
    let _pid = it.next()?;
    let port: u16 = it.next()?.parse().ok()?;
    let password = it.next()?.to_string();
    let protocol = it.next()?.to_string();
    Some(LcuCredentials { port, password, protocol })
}
```

Alternatif: process command line'dan `--app-port` / `--remoting-auth-token` oku (Riot Client restart'larında daha dayanıklı). Lockfile en basit yol.

### LCU HTTP client

```rust
let client = reqwest::Client::builder()
    .danger_accept_invalid_certs(true)   // LCU self-signed
    .http1_only()                        // LCU HTTP/1.1
    .build()?;

client.get(format!("https://127.0.0.1:{}/lol-champ-select/v1/session", port))
    .basic_auth("riot", Some(password))
    .send().await?;
```

### LCU veri uç noktaları

| Uç nokta | Kullanım |
|---|---|
| `/lol-summoner/v1/current-summoner` | Hesap, PUUID, platform |
| `/lol-gameflow/v1/gameflow-phase` | Faz takibi (ChampSelect, InProgress, ...) |
| `/lol-champ-select/v1/session` | 5v5 seçim oturumu |
| `/lol-ranked/v1/ranked-stats/{puuid}` | `queueMap` → solo/flex tier, division, LP, W-L |
| `/lol-match-history/v1/products/lol/{puuid}/matches` | Son maç listesi (özet) |
| `/lol-match-history/v1/games/{gameId}` | Tam kadro + `timeline.lane/role` |
| `/lol-lobby/v2/lobby` | Canlı lobi üyeleri ve rol tercihleri |
| `/lol-champion-mastery/v1/{puuid}/champion-mastery` | Ustalık puanları |
| `/lol-summoner/v1/summoners/{summonerId}` | Lobi üyesi adı/seviyesi |
| `/lol-game-queues/v1/queues` | Yerelleştirilmiş kuyruk adları (1 gün cache) |
| `/riotclient/region-locale` | Sunucu bölgesi (TR, EUW, ...) |

Rank seçimi: `RANKED_SOLO_5x5` → `JADE_RANKED_SOLO_5x5` → `RANKED_FLEX_SR`.
Apex tier'larda division yoktur, LP kullanılır. SR queue filtresi:
`400, 420, 430, 440, 480, 490, 700`.

### API anahtarı — yok

v0.1.1 itibarıyla Riot Web API istemcisi (`riot.rs`), `riotApiKey` ve
`platform` ayarları kaldırıldı. Sabit veri yalnızca Data Dragon'dan okunur;
kullanıcıdan hiçbir kimlik bilgisi istenmez. Bu, hem kurulum sürtünmesini
hem de anahtar sızıntısı riskini ortadan kaldırır.

v0.1.2'de isteğe bağlı deneysel OP.GG MCP modülü eklendi. Varsayılan kapalıdır,
yalnızca şampiyon adı ve pozisyon gönderir, yanıtları 24 saat yerelde cache'ler.
Çekirdek özellikler bu modül olmadan da tam çalışır.

### Champ select event (faz 2)

Polling yerine LCU WebSocket: `wss://127.0.0.1:{port}`, basic auth, `OnJsonApiEvent_lol-champ-select_v1_session` aboneliği. İlk sürümde 1 Hz phase poll yeterli; faz 2'de WS'e geç.

---

## 7. Yol Haritası

| Milestone | İçerik | Süre |
|---|---|---|
| **M0** | Tauri iskelet, tray, LCU lockfile keşfi, "client bağlı" göstergesi | 1–2 gün |
| **M1** | Champ select ekranı, 10 oyuncu kartı, rank + son maçlar, cache | 3–5 gün |
| **M2** | Post-game liste + timeline metrikleri + grafikler | 3–5 gün |
| **M3** | Overlay penceresi, ayarlar, installer + updater (tamamlandı) | 2–3 gün |
| **M4** | LCU WebSocket event'leri, testler, Developer Portal kaydı | 2–4 gün |

---

## 8. Riskler ve Önlemler

| Risk | Önlem |
|---|---|
| LCU endpoint'i patch'te değişir | Sağlayıcı katmanı + hata durumunda polling'e düş |
| LCU kapalı / giriş yapılmamış | Boş durum ekranı, son önbellek verisi |
| Maç özetinde rol boş gelir | `/games/{gameId}` detayından tamamla |
| Data Dragon erişilemez | Şampiyon adı ID'den, ikon placeholder |
| Champ select çok kısa | Cache, paralel istek, 10 dk TTL |
| Kullanıcı "MMR göster" ister | Politik olarak yasak; rank/LP göster |
| Overlay anti-cheat tarafından yanlış algılanır | Inject etme, ayrı pencere, imzalı binary |
| Kore'de LCU yasak | Platform bazlı özellik kapatma |
