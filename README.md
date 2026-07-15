# audio-dotfiles

Набор конфигураций и скриптов для настройки аудиосистемы на **CachyOS** (Arch Linux) с **PipeWire**, **EasyEffects** и оконным менеджером **niri**.

Цель — получить стабильное, качественное звучание на ноутбуке **HP Pavilion 15** с минимальным latency и максимальной прозрачностью управления.

---

## Состав репозитория

```
├── easyeffects/
│   ├── output/               # Пресеты EasyEffects (4 шт.)
│   │   ├── Perfect EQ.json         — плоский нейтральный эквалайзер
│   │   ├── Bass Enhancing.json     — бас-усиление 60 Гц
│   │   ├── Laptop_Pavilion Optimizer.json  — мультибанд + компрессор
│   │   └── HP Pavilion Safe Limiter.json   — лимитер (-1 дБ, 5 мс lookahead)
│   └── irs/                  # Импульсные отклики (4 шт.)
│       ├── Dolby Atmos.irs
│       ├── Waves MaxxAudio Pro.irs
│       ├── Razer Surround.irs
│       └── Creative X-Fi Crystalizer.irs
├── pipewire/
│   └── pipewire.conf.d/
│       └── 10-quantum.conf   # Фиксация quantum=1024, частота 48 кГц
├── installer-rs/             # Rust TUI-утилита (ratatui + crossterm)
│   ├── Cargo.toml
│   └── src/
│       ├── main.rs           # Логика: backup, deploy, rollback, systemd
│       └── ui.rs             # TUI: 3 зоны, прогресс-бар, статус-шаги
├── install.sh                # (legacy) bash-инсталлятор
├── rollback.sh               # (legacy) bash-скрипт отката
└── .gitignore
```

---

## Быстрый старт

### Способ 1: Rust TUI-бинарник (рекомендуется)

```bash
cd ~/projects/audio-dotfiles/installer-rs
cargo build --release

# Деploy с TUI-интерфейсом
./target/release/audio-manager

# Dry-run (посмотреть план без изменений)
./target/release/audio-manager --dry-run

# Rollback (восстановить из последнего бекапа)
./target/release/audio-manager --rollback
```

**Горячие клавиши в TUI:**

| Клавиша | Действие |
|---------|----------|
| `D`     | Запустить деплой |
| `R`     | Запустить откат |
| `Q` / `Esc` | Выйти |

### Способ 2: Bash-скрипты (legacy)

```bash
# Установка
bash install.sh

# Откат
bash rollback.sh
```

---

## Что делает установка

1. **Бекап** существующих конфигов в `~/.config/audio_backup_YYYYMMDD_HHMMSS.tar.gz`
2. **Симлинки** пресетов EasyEffects из репозитория в `~/.config/easyeffects/output/`
3. **Симлинки** импульсных откликов из репозитория в `~/.config/easyeffects/irs/`
4. **Симлинк** конфига PipeWire в `~/.config/pipewire/pipewire.conf.d/10-quantum.conf`
5. **Добавление** EasyEffects в автозагрузку niri (`run-on-spawn "easyeffects" "--gapplication-service"`)
6. **Рестарт** служб PipeWire: `systemctl --user restart pipewire pipewire-pulse wireplumber`

Установка **идемпотентна** — можно запускать многократно, повторные запуски не дублируют конфиги.

---

## PipeWire tuning

Файл `pipewire/pipewire.conf.d/10-quantum.conf` фиксирует:

| Параметр | Значение |
|----------|----------|
| `default.clock.rate` | 48000 Гц |
| `default.clock.allowed-rates` | [44100, 48000] |
| `default.clock.quantum` | 1024 (min/max) |

Это обеспечивает стабильный буфер без переключений и без принудительного ресемплинга.

---

## EasyEffects: HP Pavilion Safe Limiter

Лимитер с параметрами:
- Input Gain: **0 дБ**
- Threshold: **-1.0 дБ**
- Lookahead: **5 мс**
- Oversampling: **включён**

Рекомендуется размещать **последним** в цепочке эффектов (plugins_order: `["limiter#0"]`).

---

## Разработка

```bash
# Проверка стиля Rust-кода
cd installer-rs
cargo clippy -- -D warnings

# Сборка релиза
cargo build --release
```

Требования: Rust 2021+, ratatui 0.26, crossterm 0.27.

---

## Лицензия

MIT
