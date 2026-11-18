'use strict';
/* =====================================================================
   Английский перевод. Интерфейс пишется по-русски, а наблюдатель
   переводит любой текст, который появляется на странице: разметку,
   всплывашки и сообщения движка. Ключ — русская строка без лишних пробелов.
   ===================================================================== */

const I18N_EN = {
  // Заголовок, меню, нижняя панель
  'Выключен': 'Off', 'Включён': 'On',
  'Свернуть меню': 'Collapse menu', 'Поиск по настройкам (Ctrl+F)': 'Search settings (Ctrl+F)', 'Поиск по настройкам': 'Search settings',
  'Главная': 'Home', 'Пресеты': 'Presets', 'Стратегии': 'Strategies', 'Автоподбор': 'Auto-tune', 'Списки доменов': 'Domain lists',
  'Диагностика': 'Diagnostics', 'Внешний вид': 'Appearance', 'Общие настройки': 'General', 'О программе': 'About',
  'Есть несохранённые изменения': 'You have unsaved changes', 'Отменить': 'Revert', 'Сохранить': 'Save', 'Закрыть': 'Close',
  'Ничего не нашлось': 'Nothing found',

  // 0.4: списки, настройки, «Не открывается сайт», отчёт, темы
  'Списки': 'Lists', 'Настройки': 'Settings',
  'По одному адресу на строку. Поддомены включаются автоматически. Изменения сохраняются сами.': 'One address per line. Subdomains are included automatically. Changes are saved automatically.',
  'Свои домены': 'Custom domains', 'Исключения': 'Exclusions', 'Игры (IP)': 'Games (IP)',
  'Подобрать обход для них': 'Find a bypass for them',
  'Работают, когда включён пресет «Свои домены». Автоподбор ищет один обход, который открывает их все.': 'Used when the “Custom domains” preset is on. Auto-tune looks for one bypass that opens all of them.',
  'Не открывается сайт?': 'Site not opening?', 'Например: rutracker.org': 'For example: rutracker.org', 'Починить': 'Fix it',
  'Proryv добавит сайт в «Свои домены» и подберёт обход, который откроет его вместе с остальными вашими сайтами.': 'Proryv adds the site to “Custom domains” and finds a bypass that opens it together with your other sites.',
  'Введите адрес сайта, например rutracker.org': 'Enter a site address, e.g. rutracker.org',
  'Сначала добавьте сайты в «Свои домены»': 'Add sites to “Custom domains” first',
  'Отчёт для помощи': 'Support report', 'Собрать отчёт': 'Build report', 'Собираем…': 'Collecting…', 'Скопировать': 'Copy',
  'Версия, провайдер, стратегии, ошибки и какие сайты не открываются — без IP и ваших доменов. Скопируйте и отправьте в Discord.': 'Version, provider, strategies, errors and which sites fail — no IPs and no custom domains. Copy it and send it to Discord.',
  'Скопируйте и отправьте в Discord. IP-адресов и ваших доменов здесь нет.': 'Copy it and send it to Discord. It contains no IP addresses and none of your domains.',
  'Тема': 'Theme', 'Светлая, тёмная или чёрная (для OLED-экранов).': 'Light, dark or black (for OLED screens).',
  'Тёмная': 'Dark', 'Светлая': 'Light', 'Чёрная': 'Black',
  'Картинка на фоне': 'Background image', 'Выбрать…': 'Choose…', 'Убрать': 'Remove',
  'Своя картинка под интерфейсом. Хранится в настройках — исходный файл можно удалить.': 'Your own picture behind the interface. It is stored in the settings, so the original file can be deleted.',
  'Затемнение картинки': 'Image dimming', 'Чтобы текст хорошо читался.': 'So the text stays readable.',
  'Прозрачность окна': 'Window transparency', 'Mica и Acrylic работают только в Windows 11, нужен перезапуск.': 'Mica and Acrylic work only on Windows 11; restart required.',
  'Не удалось открыть картинку': 'Could not open the image',
  'Самолечение и сторож': 'Self-healing and watchdog',
  'Proryv следит за соединениями и раз в 5 минут сам проверяет сайты включённых пресетов. Если сайт перестал открываться — переподберёт стратегию (не чаще раза в 15 минут) и пришлёт уведомление.': 'Proryv watches connections and checks the sites of enabled presets every 5 minutes. If a site stops opening, it re-tunes the strategy (at most once every 15 minutes) and sends a notification.',
  'Сбросить все настройки, списки и свои стратегии?': 'Reset all settings, lists and custom strategies?',
  'Настройки сброшены': 'Settings reset', 'Настройки импортированы': 'Settings imported',
  'разрез + фейк той же длины вокруг каждого куска (fakeddisorder — куски задом наперёд)': 'split + a same-length fake around each piece (fakeddisorder — pieces in reverse order)',
  'имя сайта отдельным куском, вокруг — фейки с выдуманным именем': 'the site name as a separate piece, surrounded by fakes with a made-up name',
  'TTL фейков по числу хопов до сервера: -1:3-20 (поправка:мин-макс)': 'fake TTL from the hop count to the server: -1:3-20 (delta:min-max)',
  'как обмануть сервер: ts, badseq, badsum, md5sig, datanoack': 'how to fool the server: ts, badseq, badsum, md5sig, datanoack',
  'fake, multisplit, multidisorder, split, disorder, fakedsplit, fakeddisorder, hostfakesplit — через запятую': 'fake, multisplit, multidisorder, split, disorder, fakedsplit, fakeddisorder, hostfakesplit — comma-separated',
  'добавьте сайты в «Свои домены»': 'add sites to “Custom domains”',

  // Мастер
  'Настройка Proryv': 'Proryv setup', 'Выберите язык': 'Choose your language', 'Как в системе': 'System default', 'Русский': 'Русский',
  'Тему и фон окна можно поменять в любой момент в настройках.': 'You can change the window background at any time in settings.',
  'Фон окна': 'Window background', 'Mica и Acrylic работают только в Windows 11.': 'Mica and Acrylic work only on Windows 11.',
  'Нет': 'None', 'Акцентный цвет': 'Accent color', 'Цвет переключателей и кнопок.': 'Color of toggles and buttons.',
  'Что разблокировать?': 'What should be unblocked?', 'Выберите сервисы. Стратегию обхода программа подберёт сама.': 'Pick services. Proryv will find a bypass strategy by itself.',
  'Запуск': 'Startup', 'Запускать вместе с Windows': 'Start with Windows', 'Обход будет работать сразу после включения ПК.': 'The bypass works right after you turn on the PC.',
  'Включать обход при запуске': 'Enable bypass on launch', 'Не нужно нажимать кнопку каждый раз.': 'No need to press the button every time.',
  'Сворачивать в трей': 'Minimize to tray', 'Кнопка закрытия прячет окно, обход продолжает работать.': 'The close button hides the window, the bypass keeps working.',
  'Проверка системы': 'System check', 'Ищем программы, которые будут мешать работе.': 'Looking for programs that would interfere.',
  'Назад': 'Back', 'Далее': 'Next', 'Готово': 'Done',

  // Главная
  'Просмотр в браузере: перехват пакетов работает только в приложении.': 'Browser preview: packet interception works only in the app.',
  'Для перехвата трафика нужны права администратора.': 'Administrator rights are required to intercept traffic.',
  'Перезапустить от администратора': 'Restart as administrator', 'Обновить': 'Update',
  'Не найдены файлы WinDivert (WinDivert.dll и WinDivert64.sys) рядом с программой.': 'WinDivert files (WinDivert.dll and WinDivert64.sys) were not found next to the program.',
  'Обход выключен': 'Bypass is off', 'Обход включён': 'Bypass is on', 'Включаем…': 'Turning on…', 'Выключаем…': 'Turning off…', 'Не удалось включить': 'Failed to turn on',
  'Нажмите кнопку, чтобы включить.': 'Press the button to turn it on.', 'Сначала выберите хотя бы один пресет.': 'Select at least one preset first.',
  'Откройте заблокированный сайт — он должен загрузиться.': 'Open a blocked site — it should load now.',
  'Время работы': 'Uptime', 'Соединений': 'Connections', 'Обработано': 'Processed', 'Фейков отправлено': 'Fakes sent',
  'Провайдер': 'Internet provider', 'Определяем провайдера…': 'Detecting your provider…',
  'Для «Авто» берутся стратегии, которые сработали у других пользователей этого провайдера.': '“Auto” uses strategies that worked for other users of this provider.',
  'Не удалось определить провайдера': 'Could not detect the provider',
  'Проверка шла через VPN или без интернета. Стратегии подбираются как обычно.': 'The check went through a VPN or there is no internet. Strategies are picked as usual.',
  'есть в базе': 'in database', 'нет в базе': 'not in database',
  'Для этого провайдера в базе пока нет стратегий. Запустите автоподбор и поделитесь результатом.': 'There are no strategies for this provider yet. Run auto-tune and share the result.',

  // Пресеты
  'Включите сервисы, которые нужно разблокировать. «Авто» — программа сама подбирает и меняет стратегию, если она перестала работать.':
    'Turn on the services you need. “Auto” means Proryv picks the strategy itself and changes it if it stops working.',
  'Авто (рекомендуется)': 'Auto (recommended)', 'Дополнительно': 'Extras',
  'Локальный прокси: Telegram Desktop ходит через WebSocket веб-версии Telegram, как в TgWsProxy. Работает без VPN для аккаунтов на DC2/DC4 (большинство в России).':
    'Local proxy: Telegram Desktop connects through the WebSocket of Telegram Web, like TgWsProxy. Works without a VPN for accounts on DC2/DC4 (most accounts in Russia).',
  'Подключить Telegram': 'Connect Telegram', 'Скопировать ссылку': 'Copy link', 'Проверить': 'Check',
  'Обход для игр: фейки перед первыми пакетами соединения к игровым серверам (порты 1024–65535). Как одноимённый режим в zapret.':
    'Bypass for games: fakes before the first packets to game servers (ports 1024–65535). Same as the zapret mode.',
  'Только UDP': 'UDP only', 'Только TCP': 'TCP only', 'TCP и UDP': 'TCP and UDP',
  'Игровые серверы (диапазоны IP)': 'Game servers (IP ranges)', 'По одному на строку, формат': 'One per line, format', 'или': 'or',
  '. По умолчанию — серверы Roblox. Game Filter действует только на эти адреса, чтобы не трогать остальной трафик.':
    '. Roblox servers by default. Game Filter only touches these addresses and leaves other traffic alone.',
  'Включите и нажмите «Сохранить», чтобы запустить прокси.': 'Turn it on and press “Save” to start the proxy.',
  'Проверяем дата-центры…': 'Checking data centers…',

  // Пресеты и стратегии из движка
  'Видео, превью и мобильные клиенты YouTube.': 'Videos, thumbnails and YouTube mobile clients.',
  'Сайт, приложение, CDN и голосовые каналы (UDP).': 'Website, app, CDN and voice channels (UDP).',
  'Сайт, лаунчер и CDN Roblox.': 'Roblox website, launcher and CDN.',
  'Прочие заблокированные': 'Other blocked sites',
  'Cloudflare, Twitch-эмоуты, DoH, трекеры, SoundCloud, Medium и др.': 'Cloudflare, Twitch emotes, DoH, trackers, SoundCloud, Medium and more.',
  'Соцсети': 'Social networks', 'Instagram, Facebook, X/Twitter. Часто блокируются по IP — может не помочь.': 'Instagram, Facebook, X/Twitter. Often blocked by IP — may not help.',
  'Свои домены': 'Custom domains',
  'Только сайты из вашего списка («Списки доменов»). Пустой список — ничего не делает.': 'Only sites from your list (“Domain lists”). An empty list does nothing.',
  'Фейк к google.com, затем ClientHello режется на 1-м байте, и каждый кусок окружён фейком той же длины (fooling=ts).': 'A fake to google.com, then the ClientHello is split at byte 1 and each piece is wrapped in a same-length fake (fooling=ts).',
  'Разрез посередине домена, куски задом наперёд, вокруг каждого — фейки той же длины, которые сервер отбросит по timestamp.': 'Split in the middle of the domain, pieces in reverse order, each wrapped in same-length fakes the server drops by timestamp.',
  'Имя сайта уходит отдельным куском, а до и после него — фейки с выдуманным именем той же длины.': 'The site name goes as a separate piece, surrounded by fakes with a made-up name of the same length.',
  'Фейки без флага ACK (сервер их отбросит, DPI — нет), затем разрез посередине имени.': 'Fakes without the ACK flag (the server drops them, DPI does not), then a split in the middle of the name.',
  'Драйвер перехвата уже занят другой программой (обычно служба zapret или GoodbyeDPI). Удалите её службу (в zapret: service.bat → Remove Services) и перезагрузите ПК.': 'The capture driver is already used by another program (usually the zapret or GoodbyeDPI service). Remove its service (in zapret: service.bat → Remove Services) and restart the PC.',
  'Разрез на 2-м байте и сразу после начала SNI, наложение 679 байт фейка к google.com (general ALT7).': 'Split at byte 2 and right after the SNI start, 679-byte fake overlap to google.com (general ALT7).',
  '8 фейков, которые сервер отбросит по timestamp, затем наложение 681 байт (general ALT11).': '8 fakes the server drops by timestamp, then a 681-byte overlap (general ALT11).',
  '6 фейковых ClientHello к google.com с уменьшенным timestamp (general ALT10).': '6 fake ClientHellos to google.com with a lowered timestamp (general ALT10).',
  'Перед данными — 681 байт фейкового ClientHello к google.com «из прошлого». Основной метод zapret для YouTube.': 'Before the data — 681 bytes of a fake ClientHello to google.com “from the past”. The main zapret method for YouTube.',
  'То же наложение, 568 байт к 4pda.to. Метод zapret для Discord и остальных сайтов.': 'The same overlap, 568 bytes to 4pda.to. The zapret method for Discord and other sites.',
  '11 фейков с неверным seq, потом части задом наперёд (разрез посередине домена).': '11 fakes with a wrong seq, then the parts in reverse order (split in the middle of the domain).',
  'Режет ClientHello посередине SNI.': 'Splits the ClientHello in the middle of the SNI.',
  'Режет после 1 байта и отправляет части задом наперёд.': 'Splits after byte 1 and sends the parts in reverse order.',
  'Режет перед, в середине и после имени сайта.': 'Splits before, in the middle of and after the site name.',
  'Сначала поддельный ClientHello с малым TTL, потом настоящий с разрезом.': 'First a fake ClientHello with a low TTL, then the real one split.',
  'Фейк с неверным seq и отправка частей задом наперёд.': 'A fake with a wrong seq and the parts sent in reverse order.',
  'Для обычного HTTP: hoSt вместо Host + разрез.': 'For plain HTTP: hoSt instead of Host + split.',
  'Без обхода': 'No bypass', 'проверка без обхода': 'check without bypass',

  // Стратегии
  'Методы обхода. Посмотрите, как стратегия режет пакет: введите сайт и выберите стратегию.': 'Bypass methods. See how a strategy cuts the packet: enter a site and pick a strategy.',
  'настоящие данные': 'real data', 'фейк (до сервера не дойдёт)': 'fake (never reaches the server)',

  // Автоподбор
  'Главная фишка: программа сама находит дыру в DPI вашего провайдера и чинит обход, когда провайдер что-то меняет.':
    'The main feature: Proryv finds a hole in your provider’s DPI and repairs the bypass when the provider changes something.',
  'Подобрать стратегию': 'Find a strategy', 'Подобрать': 'Find', 'Подбираем…': 'Searching…',
  'Перебирает методы и проверяет сайты напрямую, мимо VPN. Лучшая стратегия сохранится сама. 1–3 минуты.': 'Tries methods and checks sites directly, bypassing any VPN. The best strategy is saved automatically. 1–3 minutes.',
  'Самолечение': 'Self-healing',
  'Proryv следит за соединениями: если по пресету много повторных попыток (сайт перестал открываться), он сам переподберёт стратегию — не чаще раза в 15 минут.':
    'Proryv watches connections: if a preset gets many retries (a site stopped opening), it re-tunes the strategy by itself — at most once every 15 minutes.',
  'Случайные фейки': 'Random fakes', 'Каждый фейк со случайным именем сайта и разрезом — DPI не сможет поймать их по шаблону.': 'Every fake gets a random site name and split, so DPI cannot match a pattern.',
  'Ошибка': 'Error', 'Без блокировки': 'Not blocked', 'Не нашли': 'Not found',
  'Лучшая стратегия:': 'Best strategy:', '. Она сохранена для пресета и уже применяется.': '. It is saved for the preset and already in use.',
  'Сайты открываются и без обхода — менять ничего не нужно.': 'Sites open even without a bypass — nothing to change.',
  'Ни одна стратегия не помогла. Возможно, сайт заблокирован по IP — проверьте его в «Диагностике».': 'No strategy helped. The site may be blocked by IP — check it in “Diagnostics”.',
  'Не нашёлся прямой адаптер — проверка могла идти через VPN.': 'No direct adapter found — the check may have gone through a VPN.',
  'Поделиться результатом': 'Share result', 'Подбор уже идёт': 'Auto-tune is already running',
  'Идёт подбор стратегии — подождите': 'Auto-tune is running — please wait',

  // Домены
  'По одному домену на строку. Поддомены включаются автоматически.': 'One domain per line. Subdomains are included automatically.',
  'Исключения': 'Exclusions',
  'Эти сайты никогда не обрабатываются. Банки, Госуслуги, Яндекс, VK, маркетплейсы и игры с античитом (~150 доменов) уже исключены встроенным списком — сюда добавляйте свои.':
    'These sites are never processed. Banks, government services, Yandex, VK, marketplaces and games with anti-cheat (~150 domains) are already excluded — add your own here.',

  // Диагностика
  'Почему сайт не открывается: подмена DNS, блокировка по IP или DPI.': 'Why a site does not open: DNS spoofing, IP block or DPI.',
  'Конфликты': 'Conflicts', 'VPN-туннели': 'VPN tunnels', 'Трафик через эти адаптеры Proryv не трогает — иначе ломается VPN.': 'Proryv leaves traffic on these adapters alone — otherwise the VPN breaks.',
  'Введите адрес сайта': 'Enter a site address', 'Проверяем…': 'Checking…',
  'Открывается': 'Opens', 'Блокировка DPI': 'DPI block', 'Блокировка по IP': 'IP block', 'Проблема DNS': 'DNS problem', 'Не удалось проверить': 'Could not check',
  'Конфликтов не найдено': 'No conflicts found', 'Другие программы обхода сейчас не запущены.': 'No other bypass programs are running.',
  'пропускается': 'skipped', 'VPN-туннелей нет': 'No VPN tunnels', 'Весь трафик идёт напрямую.': 'All traffic goes directly.',
  'Не удалось узнать адрес сайта (DNS). Проверьте имя сайта.': 'Could not resolve the site address (DNS). Check the site name.',
  'Адрес выдан VPN-клиентом (fake-IP) — сайт идёт через VPN, проверить его напрямую нельзя.': 'The address comes from a VPN client (fake-IP) — the site goes through the VPN and cannot be checked directly.',
  'Соединение есть, но рукопожатие TLS обрывается — это блокировка DPI. Добавьте сайт в «Свои домены» и подберите стратегию.':
    'The connection works but the TLS handshake is cut — this is a DPI block. Add the site to “Custom domains” and run auto-tune.',

  // Внешний вид
  'Требуется перезапуск. Mica работает только в Windows 11.': 'Mica works only on Windows 11.', 'Цвет переключателей, кнопок и индикаторов.': 'Color of toggles, buttons and indicators.',
  'Анимации': 'Animations', 'Отключите для слабых ПК.': 'Turn off on slow PCs.', 'Язык': 'Language', 'Язык интерфейса и уведомлений.': 'Interface and notification language.',

  // Общие
  'Импорт настроек': 'Import settings', 'Сбросить настройки': 'Reset settings', 'Экспорт настроек': 'Export settings',
  'Через Планировщик задач — без окна «Разрешить изменения» при входе.': 'Via Task Scheduler — no “Allow changes” prompt at sign-in.',
  'Запускать свёрнутым в трей': 'Start minimized to tray', 'При автозапуске окно не открывается, Proryv сидит в трее.': 'On autostart the window stays hidden, Proryv sits in the tray.',
  'Сворачивать в трей при закрытии': 'Minimize to tray on close', 'Крестик прячет окно, обход продолжает работать. Выйти — через значок в трее.': 'The X button hides the window, the bypass keeps working. Quit from the tray icon.',
  'Сеть': 'Network', 'Блокировать QUIC к YouTube': 'Block QUIC to YouTube',
  'Только UDP 443 к серверам Google/YouTube — браузер пойдёт по TCP, где работает обход. VPN и игры, которым нужен UDP 443, не затрагиваются.':
    'Only UDP 443 to Google/YouTube servers — the browser falls back to TCP, where the bypass works. VPNs and games that need UDP 443 are not affected.',
  'Прочее': 'Other', 'Проверять обновления': 'Check for updates',
  'При запуске смотреть, нет ли новой версии на GitHub, и предлагать обновиться в один клик.': 'On launch, look for a new version on GitHub and offer a one-click update.',
  'Обновлять базу стратегий': 'Update strategy database',
  'Списки доменов, стратегии и база провайдеров скачиваются с GitHub при запуске — без новой версии программы.': 'Domain lists, strategies and the provider database are downloaded from GitHub on launch — no new app version needed.',
  'Уведомления': 'Notifications', 'Уведомление Windows, когда самолечение сменило стратегию или обход не смог включиться.': 'A Windows notification when self-healing switched the strategy or the bypass failed to start.',
  'Показать мастер настройки снова': 'Show the setup wizard again', 'Открыть': 'Open',
  'Скопируйте текст и сохраните его или отправьте другу.': 'Copy the text and keep it or send it to a friend.',
  'Скопировать': 'Copy', 'Скопировано': 'Copied', 'Вставьте текст настроек, экспортированный из Proryv.': 'Paste settings exported from Proryv.',
  'Импортировать': 'Import', 'Это не похоже на настройки Proryv': 'This does not look like Proryv settings',
  'Импортировано — нажмите «Сохранить»': 'Imported — press “Save”', 'Настройки сохранены': 'Settings saved', 'Настройки сброшены — нажмите «Сохранить»': 'Settings reset — press “Save”',
  'Выберите хотя бы один пресет': 'Select at least one preset', 'В браузере окно не закрыть': 'The window cannot be closed in a browser',

  // Разработчик
  'Разработчик': 'Developer', 'Режим разработчика': 'Developer mode',
  'Вкладка «Разработчик»: стратегии в синтаксисе zapret, свои альты в автоподборе и живой лог движка.': 'The “Developer” tab: strategies in zapret syntax, your own strategies in auto-tune and a live engine log.',
  'Стратегии в синтаксисе zapret, свои альты в автоподборе и живой лог движка.': 'Strategies in zapret syntax, your own strategies in auto-tune and a live engine log.',
  'Стратегия из zapret': 'Strategy from zapret', 'Открыть .bat': 'Open .bat',
  'Вставьте параметры winws или блок из .bat — Proryv переведёт их в свою стратегию.': 'Paste winws parameters or a block from a .bat — Proryv will turn them into its own strategy.',
  'Сохранить как свою': 'Save as mine', 'Свои стратегии': 'My strategies', 'Экспорт': 'Export', 'Импорт': 'Import',
  'Участвуют в автоподборе вместе со встроенными и выбираются в пресетах.': 'They take part in auto-tune next to the built-in ones and can be picked in presets.',
  'Живой лог': 'Live log', 'запись': 'recording', 'пауза': 'paused', 'Пауза': 'Pause', 'Продолжить': 'Resume', 'Очистить': 'Clear',
  'Фильтр: сайт или стратегия': 'Filter: site or strategy',
  'Включите обход и откройте сайт — здесь появятся строки.': 'Turn the bypass on and open a site — lines will appear here.',
  'Откройте сайт — здесь появятся строки.': 'Open a site — lines will appear here.',
  'Шпаргалка по параметрам': 'Parameter cheat sheet', 'Полная документация bol-van ↗': 'Full bol-van documentation ↗',
  'fake, multisplit, multidisorder, split, disorder — режимы через запятую': 'fake, multisplit, multidisorder, split, disorder — modes, comma-separated',
  'где резать: 1, host, endhost, midsld, sniext+1 (и midhost — только в Proryv)': 'where to cut: 1, host, endhost, midsld, sniext+1 (and midhost — Proryv only)',
  'наложение: сколько байт фейка подложить перед данными': 'overlap: how many fake bytes to put before the data',
  'файл tls_clienthello_САЙТ.bin — к какому сайту фейк наложения': 'tls_clienthello_SITE.bin — which site the overlap fake is for',
  'как обмануть сервер: ts, badseq, badsum, md5sig': 'how to fool the server: ts, badseq, badsum, md5sig',
  'малый TTL фейка: пакет умрёт по дороге после DPI': 'low fake TTL: the packet dies on the way after the DPI',
  'сколько раз отправить фейк': 'how many times to send the fake',
  'файл tls_clienthello_САЙТ.bin — какой сайт в фейке': 'tls_clienthello_SITE.bin — which site is in the fake',
  'не проверялась': 'not checked', 'Удалить': 'Delete', 'соединений': 'connections', '· обработано': '· processed', '· фейков': '· fakes',
  'Скопируйте и отправьте другу или в Discord.': 'Copy it and send it to a friend or to Discord.',
  'Импорт стратегий': 'Import strategies', 'Вставьте экспорт своих стратегий из Proryv.': 'Paste an export of your Proryv strategies.',
  'Это не похоже на стратегии Proryv': 'This does not look like Proryv strategies',
  'фейк: случайный сайт': 'fake: random site', 'Не нашёл ни одного TCP-блока с --dpi-desync': 'No TCP block with --dpi-desync found',
  'у фейка нет fooling и ttl — он может дойти до сервера и сломать соединение': 'the fake has no fooling or ttl — it may reach the server and break the connection',
  '--ip-id задаётся пресетом (у YouTube уже zero) — пропущен': '--ip-id is set by the preset (YouTube already uses zero) — skipped',
  'Прочие': 'Other',

  // О программе
  'Обход DPI-блокировок с автоподбором стратегий под вашего провайдера. Написано на Rust.': 'DPI bypass with automatic strategy tuning for your provider. Written in Rust.',
  'Обновления': 'Updates', 'Новые версии ставятся прямо из программы, без скачивания вручную.': 'New versions install right from the app, no manual download.',
  'Лицензия': 'License', 'Discord-сервер': 'Discord server', 'Discord': 'Discord',
  'Новости, помощь и стратегии от других пользователей.': 'News, help and strategies from other users.',
  'Не помогло?': 'Didn’t help?',
  'Напишите в Discord: название провайдера и что не открывается — подскажем стратегию.': 'Write to us on Discord: your provider and what does not open — we will suggest a strategy.', 'Движок': 'Engine', 'Перехват пакетов': 'Packet capture',
  'У вас последняя версия.': 'You have the latest version.', 'Скачиваем обновление…': 'Downloading the update…',
  'Обновлений нет': 'No updates',

  // Telegram
  'Ссылка для Telegram': 'Link for Telegram', 'Скопируйте и откройте в Telegram.': 'Copy it and open it in Telegram.',
  'Открываем Telegram — подтвердите добавление прокси': 'Opening Telegram — confirm adding the proxy',
  'Ссылка скопирована — отправьте её себе в «Избранное» и нажмите': 'Link copied — send it to your Saved Messages and tap it',
  'Telegram Desktop не найден. Скопируйте ссылку и откройте её в Telegram вручную.': 'Telegram Desktop not found. Copy the link and open it in Telegram manually.',

  // Ошибки движка
  'Нужны права администратора: перезапустите программу от имени администратора.': 'Administrator rights required: restart the program as administrator.',
  'Перехват остановлен': 'Capture stopped', 'Запуск от имени администратора отменён': 'Running as administrator was cancelled',
};

// Строки с подстановками: [регулярное выражение, замена].
const I18N_RX = [
  [/^Последний обработанный сайт: (.+)$/, 'Last processed site: $1'],
  [/^(\d+) доменов · (.*)$/, '$1 domains · $2'], [/^Свои домены · (.*)$/, 'Custom domains · $1'],
  [/^(\d+) ч (\d+) мин$/, '$1 h $2 min'], [/^(\d+) мин (\d+) с$/, '$1 min $2 s'], [/^(\d+) с$/, '$1 s'],
  [/^Ошибка: (.+)$/, 'Error: $1'], [/^(.+) добавлен в «Свои домены» — подбираем обход…$/, '$1 added to “Custom domains” — finding a bypass…'],
  [/^Не удалось сохранить: (.+)$/, 'Could not save: $1'], [/^Найдено: (.+)$/, 'Found: $1'],
  [/^Процесс (.+) тоже перехватывает трафик\. .*$/, 'Process $1 also intercepts traffic. Turn it off before enabling Proryv, otherwise the two programs will interfere.'],
  [/^(\d+)\/(\d+) · ([\d.]+) с$/, '$1/$2 · $3 s'],
  [/^ClientHello (\d+) Б → (\d+) пакетов, всего (\d+) Б$/, 'ClientHello $1 B → $2 packets, $3 B total'],
  [/^(\d+)\. (.*) — (\d+) Б(.*)$/, (m, i, l, n, x) => `${i}. ${trPlan(l)} — ${n} B${trPlan(x)}`],
  [/^Самолечение: (.+) → «(.+)»$/, (m, p, s) => `Self-healing: ${trWord(p)} → “${trWord(s)}”`],
  [/^Самолечение: для (.*) новой стратегии не нашлось$/, (m, p) => `Self-healing: no new strategy found for ${trWord(p)}`],
  [/^Самолечение: (.+) перестал открываться — подбираем стратегию…$/, (m, p) => `Self-healing: ${trWord(p)} stopped opening — searching for a strategy…`],
  [/^Работает на (\S+) · соединений (\d+) \(всего (\d+)\) · ↑ (.+) ↓ (.+?)( · .*)?$/, (m, a, c, t, u, d, x) =>
    `Running on ${a} · connections ${c} (total ${t}) · ↑ ${trBytes(u)} ↓ ${trBytes(d)}` + (x || '').replace(/ошибок (\d+)/, 'errors $1').replace('неверный секрет — переподключите Telegram', 'wrong secret — reconnect Telegram')],
  [/^DC(\d): ✓ (\d+) мс$/, 'DC$1: ✓ $2 ms'], [/^DC(\d): ✗ (.+)$/, (m, d, e) => `DC${d}: ✗ ${trWord(e)}`],
  [/^Вышла новая версия (\S+) \(у вас (\S+)\)\.(.*)$/, 'New version $1 is out (you have $2).$3'],
  [/^Доступна версия (\S+)\.$/, 'Version $1 is available.'],
  [/^Не удалось проверить: (.+)$/, 'Could not check: $1'], [/^Не удалось обновиться: (.+)$/, 'Update failed: $1'],
  [/^Скачиваем обновление… (\d+)%$/, 'Downloading the update… $1%'], [/^Скачиваем обновление… (.+)$/, (m, b) => `Downloading the update… ${trBytes(b)}`],
  [/^Сайт открывается \(HTTP (\d+), (\d+) мс\)\.$/, 'The site opens (HTTP $1, $2 ms).'],
  [/^DNS вернул подменённый адрес \((.+)\)\. .*$/, 'DNS returned a spoofed address ($1). This is a DNS block — turn on “Secure DNS” in your browser or set Windows DNS to 1.1.1.1.'],
  [/^Сервер (.+) недоступен даже по TCP .*$/, 'Server $1 is unreachable even over TCP — looks like an IP block. A DPI bypass will not help here, you need a VPN/proxy.'],
  [/^ · проверено напрямую, мимо VPN$/, ' · checked directly, bypassing the VPN'],
  [/^Не найдены файлы WinDivert \((.+)\)\. Переустановите программу\.$/, 'WinDivert files not found ($1). Reinstall the program.'],
  [/^Windows заблокировала драйвер WinDivert \((.+)\)\. .*$/, 'Windows blocked the WinDivert driver ($1). Check your antivirus and “Memory integrity”.'],
  [/^Ошибка перехвата: (.+)$/, 'Capture error: $1'],
  [/^Не удалось настроить автозапуск: (.*)$/, 'Could not set up autostart: $1'],
  [/^(.+) · AS(\d+)$/, '$1 · AS$2'],
  [/^(.*) · (\d+) фейк\S* \+ (\d+) част\S*( · ПОВТОР — прошлый не прошёл)?$/, (m, a, f, p, r) => `${a.replace(/ \/ (.+)$/, (x, n) => ' / ' + trWord(n))} · ${f} fakes + ${p} parts` + (r ? ' · RETRY — the previous one failed' : '')],
  [/^(--[\w-]+) пока не поддерживается — пропущен$/, '$1 is not supported yet — skipped'],
  [/^режим (\S+) пока не поддерживается — пропущен$/, 'mode $1 is not supported yet — skipped'],
  [/^fooling=(\S+) не поддерживается — пропущен$/, 'fooling=$1 is not supported — skipped'],
  [/^позиция «(.+)» не поддерживается — пропущена$/, 'position «$1» is not supported — skipped'],
  [/^Добавлено: (\d+)$/, 'Added: $1'],
  [/^(YouTube|Discord|Прочие) (\d+)\/(\d+)$/, (m, p, a, b) => `${trWord(p)} ${a}/${b}`],
];

const trWord = (s) => I18N_EN[s] || s;
const trBytes = (s) => s.replace(/ ГБ$/, ' GB').replace(/ МБ$/, ' MB').replace(/ КБ$/, ' KB').replace(/ Б$/, ' B');
const trPlan = (s) => s.replace(/фейк #/g, 'fake #').replace(/часть /g, 'part ').replace(/(\d+) Б/g, '$1 B')
  ;

let LANG = 'ru';

/** Перевести одну строку (с сохранением пробелов по краям) или вернуть null. */
function trText(s) {
  if (LANG !== 'en' || !/[а-яё]/i.test(s)) return null;
  const key = s.trim().replace(/\s+/g, ' ');
  let v = I18N_EN[key];
  if (v === undefined) {
    for (const [rx, rep] of I18N_RX) if (rx.test(key)) { v = key.replace(rx, rep); break; }
  }
  if (v === undefined) return null;
  return s.match(/^\s*/)[0] + v + s.match(/\s*$/)[0];
}
/** Для строк, которые нужны в коде (не в разметке). */
function t(s) { return trText(s) ?? s; }

const I18N_ATTRS = ['placeholder', 'title'];
function trNode(n) {
  if (n.nodeType === 3) {
    if (n.parentElement && n.parentElement.closest('script, style, textarea')) return;
    if (LANG === 'en') {
      const v = trText(n.nodeValue);
      if (v !== null && v !== n.nodeValue) { n.__ru = n.nodeValue; n.nodeValue = v; }
    } else if (n.__ru !== undefined) { n.nodeValue = n.__ru; n.__ru = undefined; }
    return;
  }
  if (n.nodeType !== 1) return;
  for (const a of I18N_ATTRS) {
    const key = '__ru_' + a, val = n.getAttribute(a);
    if (val == null) continue;
    if (LANG === 'en') { const v = trText(val); if (v !== null) { n[key] = val; n.setAttribute(a, v); } }
    else if (n[key] !== undefined) { n.setAttribute(a, n[key]); n[key] = undefined; }
  }
  for (const c of n.childNodes) trNode(c);
}

function resolveLang(setting) {
  if (setting === 'ru' || setting === 'en') return setting;
  return /^ru|^be|^uk|^kk/i.test(navigator.language || 'ru') ? 'ru' : 'en';
}
function applyLang(setting) {
  LANG = resolveLang(setting);
  document.documentElement.lang = LANG;
  trNode(document.body);
}

new MutationObserver((list) => {
  if (LANG !== 'en') return;
  for (const m of list) {
    if (m.type === 'characterData') trNode(m.target);
    else if (m.type === 'attributes') trNode(m.target);
    else m.addedNodes.forEach(trNode);
  }
}).observe(document.documentElement, { subtree: true, childList: true, characterData: true, attributes: true, attributeFilter: I18N_ATTRS });
