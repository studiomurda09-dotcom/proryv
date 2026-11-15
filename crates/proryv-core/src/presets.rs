//! Встроенные пресеты: какие домены и порты обрабатывать и какими стратегиями.

use serde::{Deserialize, Serialize};

use crate::strategy::{Anchor, Desync, FakeSni, Fooling, Pos, Strategy};

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq)]
pub struct Preset {
    pub id: String,
    pub name: String,
    pub description: String,
    pub icon: String,
    pub tcp_ports: Vec<u16>,
    /// Диапазоны UDP-портов (QUIC, голос Discord и т.д.).
    pub udp_ports: Vec<(u16, u16)>,
    pub domains: Vec<String>,
    /// Стратегии в порядке перебора: первая — основная.
    pub strategies: Vec<String>,
    /// Обнулять IP ID в наших пакетах (как --ip-id=zero в zapret для Google).
    #[serde(default)]
    pub ip_id_zero: bool,
}

fn s(id: &str, name: &str, description: &str, steps: Vec<Desync>) -> Strategy {
    Strategy { id: id.into(), name: name.into(), description: description.into(), steps }
}

fn fake(ttl: u8, fooling: Vec<Fooling>, repeats: u8) -> Desync {
    Desync::Fake { ttl, fooling, sni: FakeSni::Random, repeats }
}

pub fn builtin_strategies() -> Vec<Strategy> {
    use Anchor::*;
    let google = || FakeSni::Fixed("www.google.com".into());
    vec![
        // --- Рабочие рецепты из zapret-discord-youtube ---
        s("alt7", "ALT7: наложение + разрез у SNI", "Разрез на 2-м байте и сразу после начала SNI, наложение 679 байт фейка к google.com (general ALT7).",
          vec![Desync::MultiSplit { at: vec![Pos::abs(2), Pos { anchor: SniExt, add: 1 }] },
               Desync::SeqOverlap { len: 679, sni: "www.google.com".into() }]),
        s("alt11", "ALT11: фейки (ts) + наложение", "8 фейков, которые сервер отбросит по timestamp, затем наложение 681 байт (general ALT11).",
          vec![Desync::Fake { ttl: 0, fooling: vec![Fooling::Ts], sni: google(), repeats: 8 },
               Desync::MultiSplit { at: vec![Pos::abs(1)] },
               Desync::SeqOverlap { len: 681, sni: "www.google.com".into() }]),
        s("fake_ts", "Фейки (ts)", "6 фейковых ClientHello к google.com с уменьшенным timestamp (general ALT10).",
          vec![Desync::Fake { ttl: 0, fooling: vec![Fooling::Ts], sni: google(), repeats: 6 }]),
        s("seqovl_google", "Наложение (Google)", "Перед данными — 681 байт фейкового ClientHello к google.com «из прошлого». Основной метод zapret для YouTube.",
          vec![Desync::MultiSplit { at: vec![Pos::abs(1)] }, Desync::SeqOverlap { len: 681, sni: "www.google.com".into() }]),
        s("seqovl_4pda", "Наложение (4pda)", "То же наложение, 568 байт к 4pda.to. Метод zapret для Discord и остальных сайтов.",
          vec![Desync::MultiSplit { at: vec![Pos::abs(1)] }, Desync::SeqOverlap { len: 568, sni: "4pda.to".into() }]),
        s("fake_multidisorder", "Фейки + обратный порядок", "11 фейков с неверным seq, потом части задом наперёд (разрез посередине домена).",
          vec![Desync::Fake { ttl: 0, fooling: vec![Fooling::BadSeq], sni: google(), repeats: 11 },
               Desync::MultiDisorder { at: vec![Pos::abs(1), Pos::at(Midsld)] }]),
        // md5sig и TLS-record split убраны: тест probe показал, что они рвут соединения
        // (md5-фейк принимается серверами Google; tlsrec меняет длину данных и ломает seq).
        // --- Базовые методы ---
        s("split_mid", "Разрез посередине имени", "Режет ClientHello посередине SNI.",
          vec![Desync::Split { at: Pos::at(HostMid) }]),
        s("disorder_1", "Беспорядок", "Режет после 1 байта и отправляет части задом наперёд.",
          vec![Desync::Disorder { at: Pos::abs(1) }]),
        s("multisplit_host", "Мультиразрез", "Режет перед, в середине и после имени сайта.",
          vec![Desync::MultiSplit { at: vec![Pos::abs(1), Pos::at(HostStart), Pos::at(HostMid), Pos::at(HostEnd)] }]),
        s("fake_ttl_split", "Фейк (TTL) + разрез", "Сначала поддельный ClientHello с малым TTL, потом настоящий с разрезом.",
          vec![fake(4, vec![Fooling::Ttl], 2), Desync::Split { at: Pos::at(HostMid) }]),
        s("fake_badseq_disorder", "Фейк (badseq) + беспорядок", "Фейк с неверным seq и отправка частей задом наперёд.",
          vec![fake(0, vec![Fooling::BadSeq], 3), Desync::Disorder { at: Pos::at(HostMid) }]),
        s("http_hostcase", "HTTP: регистр Host", "Для обычного HTTP: hoSt вместо Host + разрез.",
          vec![Desync::HostCase, Desync::Split { at: Pos::at(HostMid) }]),
    ]
}

pub fn builtin_presets() -> Vec<Preset> {
    let d = |v: &[&str]| v.iter().map(|x| x.to_string()).collect::<Vec<_>>();
    let st = |v: &[&str]| v.iter().map(|x| x.to_string()).collect::<Vec<_>>();
    vec![
        Preset {
            id: "youtube".into(),
            name: "YouTube".into(),
            description: "Видео, превью и мобильные клиенты YouTube.".into(),
            icon: "youtube".into(),
            tcp_ports: vec![443],
            udp_ports: vec![(443, 443)],
            // Список как в list-google.txt zapret-discord-youtube.
            domains: d(&[
                "youtube.com", "youtu.be", "youtube-nocookie.com", "youtubekids.com",
                "googlevideo.com", "ytimg.com", "ggpht.com", "yt3.googleusercontent.com",
                "youtubei.googleapis.com", "youtube.googleapis.com", "youtubeembeddedplayer.googleapis.com",
                "jnn-pa.googleapis.com", "fonts.googleapis.com", "www.gstatic.com", "play.google.com",
                "wide-youtube.l.google.com", "youtube-ui.l.google.com", "ytimg.l.google.com",
                "yt-video-upload.l.google.com", "google.ru",
            ]),
            ip_id_zero: true,
            // Порядок по тесту probe напрямую через сотовую сеть: alt11 6/6, fake_multidisorder 5/6, остальные ≤2/6.
            strategies: st(&["alt11", "fake_multidisorder", "alt7", "seqovl_google", "fake_ts", "seqovl_4pda", "multisplit_host"]),
        },
        Preset {
            id: "discord".into(),
            name: "Discord".into(),
            description: "Сайт, приложение, CDN и голосовые каналы (UDP).".into(),
            icon: "discord".into(),
            ip_id_zero: false,
            tcp_ports: vec![443, 2053, 2083, 2087, 2096, 8443],
            udp_ports: vec![(19294, 19344), (50000, 50100)],
            // Список из list-general.txt zapret-discord-youtube.
            domains: d(&[
                "discord.com", "discord.gg", "discord.app", "discord.co", "discord.design", "discord.dev",
                "discord.gift", "discord.gifts", "discord.media", "discord.new", "discord.store", "discord.status",
                "discordapp.com", "discordapp.net", "discordcdn.com", "discord-activities.com", "discordactivities.com",
                "discordmerch.com", "discordpartygames.com", "discordsays.com", "discordsez.com", "discordstatus.com",
                "dis.gd", "discord-attachments-uploads-prd.storage.googleapis.com", "stable.dl2.discordapp.net",
            ]),
            strategies: st(&["alt11", "fake_multidisorder", "seqovl_4pda", "alt7", "disorder_1"]),
        },
        Preset {
            id: "roblox".into(),
            name: "Roblox".into(),
            description: "Сайт, лаунчер и CDN Roblox.".into(),
            icon: "gamepad".into(),
            ip_id_zero: false,
            tcp_ports: vec![443],
            udp_ports: vec![],
            domains: d(&["roblox.com", "rbxcdn.com", "rbx.com", "robloxlabs.com"]),
            strategies: st(&["seqovl_4pda", "fake_multidisorder", "multisplit_host"]),
        },
        Preset {
            id: "general".into(),
            name: "Прочие заблокированные".into(),
            description: "Cloudflare, Twitch-эмоуты, DoH, трекеры, SoundCloud, Medium и др.".into(),
            icon: "list".into(),
            ip_id_zero: false,
            tcp_ports: vec![80, 443],
            udp_ports: vec![],
            domains: d(&[
                // из list-general.txt zapret
                "dns.google", "dns.quad9.net", "dns.nextdns.io", "doh.opendns.com", "doh.cleanbrowsing.org",
                "freedns.controld.com", "wikimedia-dns.org", "cloudflare-dns.com", "cloudflare-ech.com", "encryptedsni.com",
                "cloudflareaccess.com", "cloudflareapps.com", "cloudflarebolt.com", "cloudflareclient.com",
                "cloudflareinsights.com", "cloudflareok.com", "cloudflarepartners.com", "cloudflareportal.com",
                "cloudflarepreview.com", "cloudflareresolve.com", "cloudflaressl.com", "cloudflarestatus.com",
                "cloudflarestorage.com", "cloudflarestream.com", "cloudflaretest.com", "cloudfront.net",
                "zendesk.com", "frankerfacez.com", "ffzap.com", "betterttv.net", "7tv.app", "7tv.io",
                "localizeapi.com", "klipy.com", "live-video.net",
                // добавлено Proryv
                "rutracker.org", "rutracker.cc", "rutracker.net", "rutor.info", "rutor.is", "nnmclub.to", "kinozal.tv",
                "soundcloud.com", "sndcdn.com", "medium.com", "patreon.com", "patreonusercontent.com",
                "proton.me", "protonmail.com", "archive.org",
            ]),
            strategies: st(&["alt11", "fake_multidisorder", "alt7", "seqovl_4pda", "multisplit_host", "http_hostcase"]),
        },
        Preset {
            id: "social".into(),
            name: "Соцсети".into(),
            description: "Instagram, Facebook, X/Twitter. Часто блокируются по IP — может не помочь.".into(),
            icon: "globe".into(),
            ip_id_zero: false,
            tcp_ports: vec![443],
            udp_ports: vec![],
            domains: d(&[
                "instagram.com", "cdninstagram.com", "facebook.com", "fbcdn.net", "fb.com", "messenger.com",
                "x.com", "twitter.com", "twimg.com", "t.co", "threads.net", "linkedin.com", "licdn.com",
            ]),
            strategies: st(&["alt11", "fake_multidisorder", "alt7", "seqovl_4pda"]),
        },
        Preset {
            id: "universal".into(),
            name: "Свои домены".into(),
            description: "Только сайты из вашего списка («Списки доменов»). Пустой список — ничего не делает.".into(),
            icon: "globe".into(),
            ip_id_zero: false,
            tcp_ports: vec![80, 443],
            udp_ports: vec![(443, 443)],
            domains: vec![],
            strategies: st(&["alt11", "fake_multidisorder", "alt7", "seqovl_4pda", "multisplit_host", "http_hostcase"]),
        },
    ]
}

/// Встроенные исключения (из list-exclude.txt zapret): банки, госуслуги, маркетплейсы,
/// игры с античитом и т.п. Их не трогаем никогда — обход им не нужен и может навредить.
pub const BUILTIN_EXCLUDES: &[&str] = &[
    "pusher.com", "twitch.tv", "steampowered.com", "steamcommunity.com", "steamusercontent.com", "nvidia.com",
    "msi.com", "marketplace.visualstudio.com", "gallery.vsassets.io", "gallerycdn.vsassets.io", "libreoffice.org",
    "documentfoundation.org", "7-zip.org", "aida64.com", "battlepass.ru", "finargot.ru", "calibre-ebook.com",
    "ccleaner.com", "filezilla-project.org", "freecad.org", "gpg4win.org", "handbrake.fr", "sumatrapdfreader.org",
    "win-rar.com", "mail.ru", "citilink.ru", "yandex.com", "yandex.net", "yandex.org", "yandex.md", "yandex.ru",
    "yandexadexchange.net", "yandexcloud.net", "yandexcom.net", "yandexmetrica.com", "yandexwebcache.net",
    "yandexwebcache.org", "yastat.net", "yastatic-net.ru", "yastatic.net", "ya.ru", "adfox.ru", "admetrica.ru",
    "naydex.net", "rostaxi.org", "turbopages.org", "webvisor.com", "webvisor.org", "donationalerts.com", "vk.com",
    "yandex.kz", "mts.ru", "multimc.org", "dns-shop.ru", "habr.com", "3dnews.ru", "microsoft.com",
    "microsoftonline.com", "live.com", "sharepoint.com", "minecraft.net", "xboxlive.com", "akamaitechnologies.com",
    "2ip.ru", "boosty.to", "tanki.su", "lesta.ru", "korabli.su", "tanksblitz.ru", "reg.ru", "epicgames.dev",
    "epicgames.com", "unrealengine.com", "riotgames.com", "riotcdn.net", "leagueoflegends.com", "playvalorant.com",
    "gosuslugi.ru", "gov.ru", "nalog.ru", "spb.ru", "mos.ru", "vk.ru", "vk.me", "vkvideo.ru", "ok.ru", "mycdn.me",
    "okcdn.ru", "odkl.ru", "wb.ru", "geobasket.ru", "paywb.com", "rwb.ru", "wb-basket.ru", "wbbasket.ru", "wbpay.ru",
    "wibes.ru", "wildberries.ru", "ozon.by", "ozon.com", "ozon.com.by", "ozon.com.kz", "ozon.kz", "ozon.ru",
    "ozon.tm", "ozone.ru", "ozonru.me", "ozonusercontent.com", "alfabank.ru", "gazprombank.ru", "gpb.ru",
    "dbo-dengi.online", "mtsdengi.ru", "psbank.ru", "bankline.ru", "rosbank.ru", "abr.ru", "rshb.ru", "sber.ru",
    "sberbank.com", "sberbank.ru", "cdn-tinkoff.ru", "tbank-online.com", "tbank.ru", "t-bank-app.ru",
    "tochka-tech.com", "tochka.com", "vtb.ru",
    // добавлено Proryv
    "faceit.com", "battle.net", "blizzard.com", "ea.com", "ubisoft.com", "rockstargames.com",
];

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn presets_reference_existing_strategies() {
        let ids: Vec<_> = builtin_strategies().into_iter().map(|s| s.id).collect();
        for p in builtin_presets() {
            for s in &p.strategies {
                assert!(ids.contains(s), "пресет {} ссылается на неизвестную стратегию {s}", p.id);
            }
        }
    }
}
