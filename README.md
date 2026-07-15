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
│   │   └── HP Pavilion Safe Limiter.json   — EQ + exciter + autogain + лимитер + safe limiter
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
2. **Симлинки** пресетов EasyEffects из репозитория в `~/.local/share/easyeffects/output/`
3. **Симлинки** импульсных откликов из репозитория в `~/.local/share/easyeffects/irs/`
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

Полная копия пресета **Advanced Auto Gain** с дополнительным Safe Limiter в конце:

| # | Эффект | Назначение |
|---|--------|------------|
| 1 | `equalizer#0` | 30-полосный эквалайзер (RLC BT, +4 дБ на 113–358 Гц, срез на 1–1.8 кГц) |
| 2 | `exciter#0` | Возбуждение высоких частот (6%, 5.5 кГц) |
| 3 | `autogain#0` | Автоматическая нормализация громкости (target -12 дБ LUFS) |
| 4 | `limiter#0` | Основной лимитер (threshold **0 дБ**, lookahead **10 мс**, oversampling **None**) |
| 5 | `limiter#1` | **Safe Limiter** — финальная защита (threshold **-1.0 дБ**, lookahead **5 мс**) |

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
