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

## 2. Klasör Yapısı (v0.1.0)

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
│   │   ├── format.ts             # faz/queue/rol/tier formatları
│   │   ├── types.ts              # Rust modellerinin TS karşılığı
│   │   └── components/
│   │       ├── PlayerCard.svelte
│   │       └── MatchRow.svelte
│   └── routes/
│       ├── +layout.svelte
│       └── +page.svelte          # Genel / Champ Select / Maçlar / Ayarlar
├── src-tauri/
│   ├── Cargo.toml
│   ├── build.rs
│   ├── tauri.conf.json
│   ├── capabilities/default.json
│   ├── icons/                    # Tlas ikonları
│   └── src/
│       ├── lib.rs                # setup, tek örnek, tray, pencere
│       ├── main.rs
│       ├── commands.rs           # #[tauri::command] IPC + testler
│       ├── models.rs             # serde structs
│       ├── lcu.rs                # lockfile keşfi + HTTP client
│       ├── riot.rs               # rate-limited client
│       ├── db.rs                 # SQLite WAL TTL cache
│       └── config.rs             # settings.json
├── assets/tlas-icon.png
├── CHANGELOG.md
├── ARCHITECTURE.md
├── RESEARCH.md
└── UPDATER.md
```

> Planlanan ama henüz uygulanmayan modüller: LCU WebSocket event aboneliği,
> `live/` (Live Client Data API), `analysis/postgame.rs` timeline metrikleri,
> migration dosyaları. v0.1.0 bunları polling + match-v5 özetiyle karşılıyor.

---

## 3. Veri Akışı

### Champ Select (kritik yol)

```
LCU phase poll / WS event
        │  "ChampSelect"
        ▼
GET /lol-champ-select/v1/session
        │  5 ally + 5 enemy (summonerId / puuid)
        ▼
┌─────────────────────────────────────┐
│  Riot API (paralel, concurrency 6)  │
│  league-v4  → rank / LP / winrate   │
│  match-v5   → son 5 maç özeti       │
└─────────────────────────────────────┘
        │
        ▼
SQLite cache (PUUID bazlı, 5 dk TTL)
        │
        ▼
Tauri invoke → Svelte kartları
```

**Gecikme bütçesi**
| Durum | Hedef |
|---|---|
| Cache'te olan oyuncu | < 100 ms |
| Cache boş, sıcak ağ | < 1.2 s |
| Cold start + 10 oyuncu | < 2.5 s |

**Prefetch:** v0.1.0'da champ select açıldığında istekler başlar. Planlanan: lobby
oluştuğunda (`/lol-lobby/v2/lobby`) takım arkadaşlarını çözüp champ select'te sadece
eksikleri çekmek.

### Post-game

```
LCU /lol-match-history/v1/products/lol/{puuid}/matches
        │  (key gerekmez, hızlı)
        ▼
matchId listesi → SQLite
        │  kullanıcı satıra tıklar
        ▼
Riot match-v5 + timeline → analysis/postgame.rs
        │
        ▼
Metrikler: CS/dk, gold diff @10/15/20, ward, objective participation,
death positions, power spikes, takım fight katılımı
```

Post-game **tamamen offline** çalışabilir (LCU + cache). Riot API key yoksa bile son maçlar görünür; detay analiz için key gerekir.

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
| **Rate limit** | Token bucket, `X-App-Rate-Limit` / `X-Method-Rate-Limit` header'ları | 429 ≈ 0 |
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

| Kural | Uygulama |
|---|---|
| Memory yok | `ReadProcessMemory`, `OpenProcess`, `WriteProcessMemory` yasak |
| Injection yok | DLL inject, `SetWindowsHookEx`, `CreateRemoteThread` yok |
| Input yok | `SendInput`, `keybd_event`, `mouse_event` yok |
| Sadece LCU HTTPS | `lockfile` → `riot:<password>` basic auth, self-signed cert ignore |
| Sadece Live Client read | `127.0.0.1:2999`, GET |
| Otomasyon yok | auto-accept / auto-ban / auto-pick / auto-dodge / chat yok |
| Gizli bilgi yok | enemy cooldown, enemy jungle timer, fog-of-war bilgisi yok |
| MMR/ELO yok | rank/LP göster, tahmini MMR gösterme |
| Yazma minimum | Sadece kullanıcı tıklarsa rune page / item set |
| Overlay injectsiz | Ayrı always-on-top pencere veya OBS browser source |

**Kore:** LCU tabanlı özellikleri KR platformunda kapat (`platformId == "KR"`).

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

### Riot API client

- Base: platform (`tr1`, `euw1`, ...) ve regional (`europe`, `asia`, ...) ayrımı.
- Header'lardan limit öğren: `x-app-rate-limit: 20:1,100:120`.
- 429 → `Retry-After` kadar bekle, jitter ekle.
- `match-v5` için `europe` / `americas` / `asia` / `sea` doğru seçilmeli.

### API key saklama

Düz metin dosya **kullanma**. Windows Credential Manager (`keyring` crate) veya DPAPI (`CryptProtectData`). Anahtar sadece Rust tarafında; frontend'e asla gönderme.

### Champ select event (faz 2)

Polling yerine LCU WebSocket: `wss://127.0.0.1:{port}`, basic auth, `OnJsonApiEvent_lol-champ-select_v1_session` aboneliği. İlk sürümde 1 Hz phase poll yeterli; faz 2'de WS'e geç.

---

## 7. Yol Haritası

| Milestone | İçerik | Süre |
|---|---|---|
| **M0** | Tauri iskelet, tray, LCU lockfile keşfi, "client bağlı" göstergesi | 1–2 gün |
| **M1** | Champ select ekranı, 10 oyuncu kartı, rank + son maçlar, cache | 3–5 gün |
| **M2** | Post-game liste + timeline metrikleri + grafikler | 3–5 gün |
| **M3** | Overlay penceresi, ayarlar, key saklama, installer + updater | 2–3 gün |
| **M4** | Rate limiter sertleştirme, testler, PBE, Developer Portal kaydı | 2–4 gün |

---

## 8. Riskler ve Önlemler

| Risk | Önlem |
|---|---|
| LCU endpoint'i patch'te değişir | Sağlayıcı katmanı + hata durumunda polling'e düş |
| Riot API key reddedilir | LCU-only degraded mode |
| Production key gecikir | Kişisel key + yerel cache ile başla |
| Champ select çok kısa | Lobby'de prefetch, cache, paralel istek |
| Kullanıcı "MMR göster" ister | Politik olarak yasak; rank/LP göster |
| Overlay anti-cheat tarafından yanlış algılanır | Inject etme, ayrı pencere, imzalı binary |
| Kore'de LCU yasak | Platform bazlı özellik kapatma |
