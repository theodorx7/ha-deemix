<div align="center">
  <a href="https://github.com/theodorx7/ha-deemix">
    <img src="deemix/logo.png" alt="Logo" width="180" style="vertical-align: middle; margin-right: 15px;">
  </a>
  <h1 style="display: inline-block; vertical-align: middle; margin: 0;">
    Home Assistant App: Deemix with telegram bot
  </h1>
</div>

<div align="left">
 <img src="https://img.shields.io/badge/aarch64-yes-green.svg" alt="Supports aarch64 Architecture"> &thinsp; <img src="https://img.shields.io/badge/amd64-yes-green.svg" alt="Supports amd64 Architecture">
</div>

<div align="right">
  <a href="#donate"><img src="https://img.shields.io/static/v1?label=DONATE&message=USDT%20&labelColor=555&color=26A17B&style=for-the-badge" alt="DONATE USDT"></a> &thinsp; <a href="https://donate.stream/donate_6a8404d5ea133"><img src="https://img.shields.io/badge/DONAT.stream-fc0?style=for-the-badge&logo=heart&logoColor=white" alt="DONAT.stream"></a>
</div>

[English](https://github.com/theodorx7/ha-deemix/blob/main/README.md) | [Russian](https://github.com/theodorx7/ha-deemix/blob/main/README_RU.md)

Проект обеспечивает полноценную работу оригинального (немодифицированного) [Deemix](https://github.com/bambanah/deemix) в виде приложения для Home Assistant и расширяет возможности с помощью Telegram-бота.



## Функции
- Скачивание музыки во FLAC (lossless) и MP3 (320 kbps / 128 kbps) – требуется аккаунт Deezer (для скачивания FLAC и MP3 320 необходима платная подписка)  

- Пакетное скачивание:
  - один или несколько треков выборочно из альбома
  - альбом или плейлист целиком
  - вся дискография исполнителя  

- Поиск и скачивание музыки через WebUI Deemix или Telegram-бот

### Telegram-бот (опционально)
- Открытый или приватный доступ к боту по белому списку Telegram ID  

- Скачивание по ссылке — просто отправьте её в чат:
  - Deezer: трек / альбом / плейлист / исполнитель
  - Spotify: трек / альбом / плейлист
  - Apple Music: трек / альбом  

- Распознавание музыки через ACRCloud (аналог Shazam) – [требуется API ключ](https://console.acrcloud.com/)  

- Поиск треков или альбомов по текстовому сообщению в чат  

- Очередь загрузок:
  - проверка статуса: скачивается / в ожидании / завершено / с ошибками
  - уведомления об ошибках скачивания (только тому, кто запросил загрузку)
  - очистка успешно завершённых загрузок  

- Переключение качества FLAC → MP3 320 → MP3 128  

- Обновление ARL на сервере Deemix



## Установка
### Нажмите на кнопку
[![Add repository to Home Assistant](https://my.home-assistant.io/badges/supervisor_add_addon_repository.svg)](
https://my.home-assistant.io/redirect/supervisor_add_addon_repository/?repository_url=https://github.com/theodorx7/ha-deemix/
)
### Или выполните шаги вручную
1. В интерфейсе Home Assistant перейдите в <kbd>Настройки</kbd> → <kbd>Приложения</kbd> → <kbd>Установить приложение</kbd> (внизу справа).
2. Нажмите на меню с тремя точками в правом верхнем углу <kbd>⋮</kbd> → <kbd>Репозитории</kbd> и добавьте URL-адрес этого репозитория: [https://github.com/theodorx7/ha-deemix](https://github.com/theodorx7/ha-deemix)
3. Обновите страницу и найдите приложение «Navidrome Rating Sync».



## Как открыть панель  
- Через Ingress — на странице аддона включите опцию "Показывать на боковой панели" для быстрого доступа.  
- По IP адресу на котором работает Home Assistant (только HTTP протокол), порт по умолчанию 6595 (можно поменять в настройках аддона), например: `http://192.168.1.30:6595`  



## Конфигурация
- Аддон по умолчанию запускается с правами 1000:1000, в случае индивидуальной конфигурации можно указать свои PUID и PGID через env_vars.  

- Download Path настроенный через env_vars в настройках аддона применяется только при первом запуске, далее меняется а только через WebUI Deemix.  

- Для переопределения качества загрузок через Telegram-бот используйте переменную `BOT_BITRATE` с этими вариантами значений: "1"= MP3 128, "3"=MP3 320, "9"=FLAC. Настройка качества по умолчанию — FLAC (9).

```yaml
env_vars:
  - name: BOT_BITRATE
    value: "3"
```

- Конфиги Deemix и Telegram-бота хранятся по пути: addon_configs/`<your app slug>`_deemix/config.json.  



## Настройка распознавания треков — ACRCloud  
Зарегистрируйте аккаунт в сервисе ACRCloud -> https://console.acrcloud.com  
Откройте ACRCloud Console  -> Раздел "Audio & Video Recognition":  
1. Справа в верхнем меню выберите один из 3 регионов Europe, US West или Asia Pacific 

2. Слева в боковом меню нажмите на "Projects" -> Откройте раздел "Audio & Video Recognition" -> Кнопка "Create Project"  

3. В форме создания проекта заполните поля:  
	Project Name -> Telegram Bot  
	Audio Source -> Recorded Audio (Audio captured via microphone or noisy audio files)  
	Audio Engine -> Audio Fingerprinting  
	Buckets -> ACRCloud Music  
	The 3rd Party ID Integration -> `spotify` + `deezer` + `isrc`   

⚠️ КРИТИЧНО: в опции «3rd Party ID Integration» нужно отметить галочкой 3 варианта: `deezer`, `spotify`, а также `isrc`.

4. После создания проекта в консоле ACRCloud появятся данные "Host", "Access Key", "Access Secret", заполните ими соответсвующие поля в опциях аддона:  
`ACRCloud Host`  
`ACRCloud Access Key`  
`ACRCloud Secret Key`  
