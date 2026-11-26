//! Translated strings for Remote transcription, Remote devices, and
//! Transcribe again in transforms, kept beside [`super::TRANSLATIONS`] so the
//! main table stays under the source cap.

use std::collections::HashMap;

use super::screenshots::Language;

/// One language's table with upstream's extra strings and this fork's.
pub(super) fn merged(
    table: &'static [(&'static str, &'static str)],
    language: Language,
) -> HashMap<&'static str, &'static str> {
    let fork = match language {
        Language::French => FRENCH,
        Language::Portuguese => PORTUGUESE,
        Language::Russian => RUSSIAN,
        Language::Hungarian => HUNGARIAN,
    };
    let mut merged = super::screenshots::merged(table, language);
    merged.extend(fork.iter().copied());
    merged
}

/// Columns: Polish, Chinese (Simplified), Japanese, German, Spanish, Ukrainian.
#[rustfmt::skip]
pub(super) const TRANSLATIONS: &[(&str, [&str; 6])] = &[
    ("Remote transcription", ["Transkrypcja zdalna", "远程转写", "リモート文字起こし", "Entfernte Transkription", "Transcripción remota", "Віддалене транскрибування"]),
    ("Send dictations, meeting audio, and Annotate narration to another Hex, which transcribes them and runs its Output steps. When it can't be reached, this Mac does", ["Wysyła dyktowanie, dźwięk spotkań i narrację Annotate do innego Hex, który je transkrybuje i wykonuje swoje kroki Wyjścia. Gdy jest nieosiągalny, robi to ten Mac", "将听写、会议音频和 Annotate 旁白发送到另一台 Hex，由它转写并运行其输出步骤。无法连接时，由这台 Mac 处理", "音声入力、会議の音声、Annotate のナレーションを別の Hex に送り、そちらで文字起こしと出力ステップを実行します。接続できないときはこの Mac が処理します", "Sendet Diktate, Meeting-Audio und Annotate-Kommentare an ein anderes Hex, das sie transkribiert und seine Ausgabe-Schritte ausführt. Ist es nicht erreichbar, übernimmt dieser Mac", "Envía los dictados, el audio de reuniones y la narración de Annotate a otro Hex, que los transcribe y ejecuta sus pasos de Salida. Si no está disponible, lo hace este Mac", "Надсилає диктування, звук зустрічей і коментарі Annotate до іншого Hex, який їх транскрибує та виконує свої кроки виводу. Якщо він недоступний, це робить цей Mac"]),
    ("Connecting stopped.", ["Łączenie zostało przerwane.", "连接已中断。", "接続が中断しました。", "Die Verbindung wurde abgebrochen.", "La conexión se detuvo.", "Підключення перервано."]),
    ("Disconnect", ["Rozłącz", "断开连接", "接続を解除", "Trennen", "Desconectar", "Відключити"]),
    ("Sends to {}", ["Wysyła do {}", "发送到 {}", "{} に送信", "Sendet an {}", "Envía a {}", "Надсилає до {}"]),
    ("Address or pair link", ["Adres lub link parowania", "地址或配对链接", "アドレスまたはペアリングリンク", "Adresse oder Kopplungslink", "Dirección o enlace de emparejamiento", "Адреса або посилання для створення пари"]),
    ("Code", ["Kod", "代码", "コード", "Code", "Código", "Код"]),
    ("Connect", ["Połącz", "连接", "接続", "Verbinden", "Conectar", "Підключити"]),
    ("On the other Hex, open Labs and choose Pair a device", ["W drugim Hex otwórz Labs i wybierz Sparuj urządzenie", "在另一台 Hex 上打开实验室并选择配对设备", "もう一方の Hex で Labs を開き、デバイスをペアリングを選びます", "Öffne im anderen Hex Labs und wähle Gerät koppeln", "En el otro Hex, abre Labs y elige Emparejar un dispositivo", "В іншому Hex відкрийте Labs і виберіть Створити пару з пристроєм"]),
    ("Connect to", ["Połącz z", "连接到", "接続先", "Verbinden mit", "Conectar con", "Підключитися до"]),
    ("Enter the other Hex's address and code.", ["Wpisz adres i kod drugiego Hex.", "请输入另一台 Hex 的地址和代码。", "もう一方の Hex のアドレスとコードを入力してください。", "Gib Adresse und Code des anderen Hex ein.", "Introduce la dirección y el código del otro Hex.", "Введіть адресу й код іншого Hex."]),
    ("Pair a device", ["Sparuj urządzenie", "配对设备", "デバイスをペアリング", "Gerät koppeln", "Emparejar un dispositivo", "Створити пару з пристроєм"]),
    ("Transcribe for other devices", ["Transkrybuj dla innych urządzeń", "为其他设备转写", "ほかのデバイスのために文字起こし", "Für andere Geräte transkribieren", "Transcribir para otros dispositivos", "Транскрибувати для інших пристроїв"]),
    ("Hex on your iPhone or another Mac sends its dictations here, and this Hex transcribes them and runs its Output steps", ["Hex na iPhonie lub innym Macu wysyła tu dyktowanie, a ten Hex je transkrybuje i wykonuje swoje kroki Wyjścia", "你 iPhone 或另一台 Mac 上的 Hex 会把听写发送到这里，由这台 Hex 转写并运行其输出步骤", "iPhone や別の Mac の Hex が音声入力をここに送り、この Hex が文字起こしと出力ステップを実行します", "Hex auf deinem iPhone oder einem anderen Mac sendet seine Diktate hierher, und dieses Hex transkribiert sie und führt seine Ausgabe-Schritte aus", "Hex en tu iPhone u otro Mac envía aquí sus dictados, y este Hex los transcribe y ejecuta sus pasos de Salida", "Hex на вашому iPhone або іншому Mac надсилає сюди диктування, а цей Hex їх транскрибує та виконує свої кроки виводу"]),
    ("Revoke", ["Odwołaj", "撤销", "取り消す", "Widerrufen", "Revocar", "Відкликати"]),
    ("Remote devices", ["Urządzenia zdalne", "远程设备", "リモートデバイス", "Entfernte Geräte", "Dispositivos remotos", "Віддалені пристрої"]),
    ("Address {}", ["Adres {}", "地址 {}", "アドレス {}", "Adresse {}", "Dirección {}", "Адреса {}"]),
    ("Code {}", ["Kod {}", "代码 {}", "コード {}", "Code {}", "Código {}", "Код {}"]),
    ("Scan with Hex on your iPhone, or enter both on the other Mac. The code works once, for five minutes.", ["Zeskanuj w Hex na iPhonie albo wpisz oba na drugim Macu. Kod działa raz, przez pięć minut.", "用 iPhone 上的 Hex 扫描，或在另一台 Mac 上输入两者。代码只能使用一次，有效期五分钟。", "iPhone の Hex でスキャンするか、別の Mac で両方を入力します。コードは 1 回だけ、5 分間有効です。", "Scanne mit Hex auf deinem iPhone oder gib beides auf dem anderen Mac ein. Der Code gilt einmal, für fünf Minuten.", "Escanéalo con Hex en tu iPhone o introduce ambos en el otro Mac. El código sirve una vez, durante cinco minutos.", "Скануйте в Hex на iPhone або введіть обидва значення на іншому Mac. Код діє один раз, протягом п'яти хвилин."]),
    ("Restart Hex to start serving devices.", ["Uruchom Hex ponownie, aby zacząć obsługiwać urządzenia.", "重新启动 Hex 以开始为设备提供服务。", "デバイスへの提供を始めるには Hex を再起動してください。", "Starte Hex neu, um Geräte zu bedienen.", "Reinicia Hex para empezar a atender dispositivos.", "Перезапустіть Hex, щоб почати обслуговувати пристрої."]),
    ("Editing {}'s steps", ["Edytujesz kroki: {}", "正在编辑 {} 的步骤", "{} のステップを編集中", "Schritte von {} bearbeiten", "Editando los pasos de {}", "Редагування кроків {}"]),
    ("Remote transcription is on, so dictations use the steps and vocabulary on that Hex. Changes here, Edit with agent, and hex settings on this Mac change them there",
        ["Zdalna transkrypcja jest włączona, więc dyktowanie korzysta z kroków i słownictwa na tym Hex. Zmiany tutaj, Edytuj z agentem i hex settings na tym Macu zmieniają je tam", "远程转写已开启，因此听写使用那台 Hex 上的步骤和词汇。此处的更改、“用智能体编辑”以及此 Mac 上的 hex settings 都会在那里更改它们", "リモート文字起こしがオンなので、音声入力はその Hex のステップと語彙を使います。ここでの変更、エージェントで編集、この Mac の hex settings はそちらを変更します", "Remote-Transkription ist an, daher nutzen Diktate die Schritte und das Vokabular dieses Hex. Änderungen hier, Mit Agent bearbeiten und hex settings auf diesem Mac ändern sie dort", "La transcripción remota está activada, así que los dictados usan los pasos y el vocabulario de ese Hex. Los cambios aquí, Editar con agente y hex settings en este Mac los cambian allí", "Віддалену транскрипцію ввімкнено, тож диктування використовує кроки й словник того Hex. Зміни тут, Редагувати з агентом і hex settings на цьому Mac змінюють їх там"]),
    ("Its Output steps aren't copied yet, so a fallback here uses this Mac's own", ["Jego kroki Wyjścia nie zostały jeszcze skopiowane, więc awaryjnie ten Mac używa własnych", "尚未复制它的输出步骤，因此这里的后备使用这台 Mac 自己的步骤", "出力ステップはまだコピーされていないため、ここでの代替処理はこの Mac 自身のものを使います", "Seine Ausgabe-Schritte sind noch nicht kopiert, daher nutzt ein Ausweichen hier die dieses Macs", "Sus pasos de Salida aún no se han copiado, así que el respaldo aquí usa los de este Mac", "Його кроки виводу ще не скопійовано, тож резервний варіант тут використовує власні кроки цього Mac"]),
    ("When it can't be reached, this Mac uses a copy of its Output steps and vocabulary", ["Gdy jest nieosiągalny, ten Mac używa kopii jego kroków Wyjścia i słownictwa", "无法连接时，这台 Mac 使用它的输出步骤和词汇的副本", "接続できないときは、この Mac がその出力ステップと語彙のコピーを使います", "Ist es nicht erreichbar, nutzt dieser Mac eine Kopie seiner Ausgabe-Schritte und seines Vokabulars", "Si no está disponible, este Mac usa una copia de sus pasos de Salida y su vocabulario", "Коли він недоступний, цей Mac використовує копію його кроків виводу та словника"]),
    ("It transcribes with {theirs}, this Mac with {ours}. Choose the same model in General for matching text", ["Tamten transkrybuje za pomocą {theirs}, ten Mac za pomocą {ours}. Wybierz ten sam model w Ogólnych, aby tekst był zgodny", "它使用 {theirs} 转写，这台 Mac 使用 {ours}。在通用中选择相同的模型以获得一致的文本", "向こうは {theirs}、この Mac は {ours} で文字起こしします。一致したテキストにするには一般で同じモデルを選んでください", "Es transkribiert mit {theirs}, dieser Mac mit {ours}. Wähle unter Allgemein dasselbe Modell für gleichen Text", "Transcribe con {theirs}; este Mac, con {ours}. Elige el mismo modelo en General para obtener el mismo texto", "Він транскрибує за допомогою {theirs}, цей Mac — {ours}. Виберіть ту саму модель у Загальних, щоб текст збігався"]),
    ("Transcribe again in transforms", ["Ponowna transkrypcja w przekształceniach", "在转换中重新转写", "変換で再び文字起こし", "Erneut transkribieren in Transformationen", "Volver a transcribir en transformaciones", "Повторна транскрипція в перетвореннях"]),
    ("Your transformations can hear a dictation again with another speech model", ["Twoje przekształcenia mogą ponownie przesłuchać dyktowanie innym modelem mowy", "你的转换可以用另一个语音模型重新识别一次听写", "変換で、音声入力をもう一度別の音声モデルで聞き直せます", "Deine Transformationen können ein Diktat mit einem anderen Sprachmodell erneut hören", "Tus transformaciones pueden volver a escuchar un dictado con otro modelo de voz", "Ваші перетворення можуть ще раз прослухати диктування іншою моделлю мовлення"]),
    ("Transcribe for your iPhone or another Mac", ["Transkrybuj dla iPhone'a lub innego Maca", "为你的 iPhone 或另一台 Mac 转写", "iPhone や別の Mac のために文字起こし", "Für dein iPhone oder einen anderen Mac transkribieren", "Transcribe para tu iPhone u otro Mac", "Транскрибувати для iPhone або іншого Mac"]),
    ("Too many edits waiting to reach the other Hex; try again", ["Zbyt wiele zmian czeka na wysłanie do drugiego Hex; spróbuj ponownie", "等待发送到另一台 Hex 的修改太多，请重试", "もう一方の Hex に送る変更が多すぎます。もう一度お試しください", "Zu viele Änderungen warten auf das andere Hex; versuche es erneut", "Demasiados cambios esperan llegar al otro Hex; inténtalo de nuevo", "Забагато змін очікує на надсилання до іншого Hex; спробуйте ще раз"]),
    ("Sending edits stopped", ["Wysyłanie zmian zostało przerwane", "发送修改已中断", "変更の送信が中断しました", "Das Senden der Änderungen wurde abgebrochen", "El envío de cambios se detuvo", "Надсилання змін перервано"]),
    ("Remote transcription is off, so the edit wasn't sent", ["Transkrypcja zdalna jest wyłączona, więc zmiana nie została wysłana", "远程转写已关闭，因此未发送修改", "リモート文字起こしがオフのため、変更は送信されませんでした", "Entfernte Transkription ist aus, daher wurde die Änderung nicht gesendet", "La transcripción remota está desactivada, así que el cambio no se envió", "Віддалене транскрибування вимкнено, тому зміну не надіслано"]),
    ("{machine} refused the edit: {message}", ["{machine} odrzucił zmianę: {message}", "{machine} 拒绝了修改：{message}", "{machine} が変更を拒否しました: {message}", "{machine} hat die Änderung abgelehnt: {message}", "{machine} rechazó el cambio: {message}", "{machine} відхилив зміну: {message}"]),
];

/// Russian for the strings above, chained into [`super::russian::table`].
#[rustfmt::skip]
pub(super) const RUSSIAN: &[(&str, &str)] = &[
    ("Remote transcription", "Удалённая транскрипция"),
    ("Send dictations, meeting audio, and Annotate narration to another Hex, which transcribes them and runs its Output steps. When it can't be reached, this Mac does", "Отправляет диктовки, звук встреч и комментарии Annotate на другой Hex, который их транскрибирует и выполняет свои шаги вывода. Если он недоступен, это делает этот Mac"),
    ("Connecting stopped.", "Подключение прервано."),
    ("Disconnect", "Отключить"),
    ("Sends to {}", "Отправляет на {}"),
    ("Address or pair link", "Адрес или ссылка для сопряжения"),
    ("Code", "Код"),
    ("Connect", "Подключить"),
    ("On the other Hex, open Labs and choose Pair a device", "На другом Hex откройте Labs и выберите «Сопрячь устройство»"),
    ("Connect to", "Подключиться к"),
    ("Enter the other Hex's address and code.", "Введите адрес и код другого Hex."),
    ("Pair a device", "Сопрячь устройство"),
    ("Transcribe for other devices", "Транскрибировать для других устройств"),
    ("Hex on your iPhone or another Mac sends its dictations here, and this Hex transcribes them and runs its Output steps", "Hex на вашем iPhone или другом Mac отправляет сюда диктовки, а этот Hex транскрибирует их и выполняет свои шаги вывода"),
    ("Revoke", "Отозвать"),
    ("Remote devices", "Удалённые устройства"),
    ("Address {}", "Адрес {}"),
    ("Code {}", "Код {}"),
    ("Scan with Hex on your iPhone, or enter both on the other Mac. The code works once, for five minutes.", "Отсканируйте в Hex на iPhone или введите оба значения на другом Mac. Код действует один раз, пять минут."),
    ("Restart Hex to start serving devices.", "Перезапустите Hex, чтобы начать обслуживать устройства."),
    ("Editing {}'s steps", "Редактирование шагов {}"),
    ("Remote transcription is on, so dictations use the steps and vocabulary on that Hex. Changes here, Edit with agent, and hex settings on this Mac change them there", "Удалённая транскрипция включена, поэтому диктовки используют шаги и словарь того Hex. Изменения здесь, «Редактировать с агентом» и hex settings на этом Mac меняют их там"),
    ("Its Output steps aren't copied yet, so a fallback here uses this Mac's own", "Его шаги вывода ещё не скопированы, поэтому при резервной обработке этот Mac использует свои"),
    ("When it can't be reached, this Mac uses a copy of its Output steps and vocabulary", "Если он недоступен, этот Mac использует копию его шагов вывода и словаря"),
    ("It transcribes with {theirs}, this Mac with {ours}. Choose the same model in General for matching text", "Он транскрибирует с помощью {theirs}, этот Mac — с помощью {ours}. Выберите ту же модель в разделе «Общие», чтобы текст совпадал"),
    ("Transcribe again in transforms", "Повторная транскрипция в преобразованиях"),
    ("Your transformations can hear a dictation again with another speech model", "Ваши преобразования могут ещё раз прослушать диктовку другой моделью речи"),
    ("Transcribe for your iPhone or another Mac", "Транскрибировать для iPhone или другого Mac"),
    ("Too many edits waiting to reach the other Hex; try again", "Слишком много изменений ждут отправки на другой Hex; попробуйте ещё раз"),
    ("Sending edits stopped", "Отправка изменений прервана"),
    ("Remote transcription is off, so the edit wasn't sent", "Удалённая транскрипция выключена, поэтому изменение не отправлено"),
    ("{machine} refused the edit: {message}", "{machine} отклонил изменение: {message}"),
];

/// French for the strings above, chained into [`super::french::table`].
#[rustfmt::skip]
pub(super) const FRENCH: &[(&str, &str)] = &[
    ("Remote transcription", "Transcription à distance"),
    ("Send dictations, meeting audio, and Annotate narration to another Hex, which transcribes them and runs its Output steps. When it can't be reached, this Mac does", "Envoie les dictées, l'audio des réunions et la narration d'Annotate à un autre Hex, qui les transcrit et applique ses étapes de Sortie. S'il est injoignable, ce Mac s'en charge"),
    ("Connecting stopped.", "La connexion s'est arrêtée."),
    ("Disconnect", "Déconnecter"),
    ("Sends to {}", "Envoie à {}"),
    ("Address or pair link", "Adresse ou lien d'appairage"),
    ("Code", "Code"),
    ("Connect", "Connecter"),
    ("On the other Hex, open Labs and choose Pair a device", "Sur l'autre Hex, ouvrez Labs et choisissez Appairer un appareil"),
    ("Connect to", "Se connecter à"),
    ("Enter the other Hex's address and code.", "Saisissez l'adresse et le code de l'autre Hex."),
    ("Pair a device", "Appairer un appareil"),
    ("Transcribe for other devices", "Transcrire pour d'autres appareils"),
    ("Hex on your iPhone or another Mac sends its dictations here, and this Hex transcribes them and runs its Output steps", "Hex sur votre iPhone ou un autre Mac envoie ses dictées ici, et ce Hex les transcrit et applique ses étapes de Sortie"),
    ("Revoke", "Révoquer"),
    ("Remote devices", "Appareils distants"),
    ("Address {}", "Adresse {}"),
    ("Code {}", "Code {}"),
    ("Scan with Hex on your iPhone, or enter both on the other Mac. The code works once, for five minutes.", "Scannez avec Hex sur votre iPhone, ou saisissez les deux sur l'autre Mac. Le code fonctionne une fois, pendant cinq minutes."),
    ("Restart Hex to start serving devices.", "Redémarrez Hex pour commencer à servir les appareils."),
    ("Editing {}'s steps", "Modification des étapes de {}"),
    ("Remote transcription is on, so dictations use the steps and vocabulary on that Hex. Changes here, Edit with agent, and hex settings on this Mac change them there", "La transcription à distance est activée, donc les dictées utilisent les étapes et le vocabulaire de ce Hex. Les modifications ici, Modifier avec l'agent et hex settings sur ce Mac les changent là-bas"),
    ("Its Output steps aren't copied yet, so a fallback here uses this Mac's own", "Ses étapes de Sortie ne sont pas encore copiées : en secours, ce Mac utilise les siennes"),
    ("When it can't be reached, this Mac uses a copy of its Output steps and vocabulary", "S'il est injoignable, ce Mac utilise une copie de ses étapes de Sortie et de son vocabulaire"),
    ("It transcribes with {theirs}, this Mac with {ours}. Choose the same model in General for matching text", "Il transcrit avec {theirs}, ce Mac avec {ours}. Choisissez le même modèle dans Général pour un texte identique"),
    ("Transcribe again in transforms", "Retranscrire dans les transformations"),
    ("Your transformations can hear a dictation again with another speech model", "Vos transformations peuvent réécouter une dictée avec un autre modèle vocal"),
    ("Transcribe for your iPhone or another Mac", "Transcrire pour votre iPhone ou un autre Mac"),
    ("Too many edits waiting to reach the other Hex; try again", "Trop de modifications attendent d'atteindre l'autre Hex ; réessayez"),
    ("Sending edits stopped", "L'envoi des modifications s'est arrêté"),
    ("Remote transcription is off, so the edit wasn't sent", "La transcription à distance est désactivée, la modification n'a donc pas été envoyée"),
    ("{machine} refused the edit: {message}", "{machine} a refusé la modification : {message}"),
];

/// Brazilian Portuguese for the strings above, chained into [`super::portuguese::table`].
#[rustfmt::skip]
pub(super) const PORTUGUESE: &[(&str, &str)] = &[
    ("Remote transcription", "Transcrição remota"),
    ("Send dictations, meeting audio, and Annotate narration to another Hex, which transcribes them and runs its Output steps. When it can't be reached, this Mac does", "Envia os ditados, o áudio de reuniões e a narração do Annotate para outro Hex, que os transcreve e executa suas etapas de Saída. Quando ele não está acessível, este Mac faz isso"),
    ("Connecting stopped.", "A conexão parou."),
    ("Disconnect", "Desconectar"),
    ("Sends to {}", "Envia para {}"),
    ("Address or pair link", "Endereço ou link de pareamento"),
    ("Code", "Código"),
    ("Connect", "Conectar"),
    ("On the other Hex, open Labs and choose Pair a device", "No outro Hex, abra Labs e escolha Parear um dispositivo"),
    ("Connect to", "Conectar a"),
    ("Enter the other Hex's address and code.", "Digite o endereço e o código do outro Hex."),
    ("Pair a device", "Parear um dispositivo"),
    ("Transcribe for other devices", "Transcrever para outros dispositivos"),
    ("Hex on your iPhone or another Mac sends its dictations here, and this Hex transcribes them and runs its Output steps", "O Hex no seu iPhone ou em outro Mac envia os ditados para cá, e este Hex os transcreve e executa suas etapas de Saída"),
    ("Revoke", "Revogar"),
    ("Remote devices", "Dispositivos remotos"),
    ("Address {}", "Endereço {}"),
    ("Code {}", "Código {}"),
    ("Scan with Hex on your iPhone, or enter both on the other Mac. The code works once, for five minutes.", "Escaneie com o Hex no seu iPhone ou digite ambos no outro Mac. O código funciona uma vez, por cinco minutos."),
    ("Restart Hex to start serving devices.", "Reinicie o Hex para começar a atender dispositivos."),
    ("Editing {}'s steps", "Editando as etapas de {}"),
    ("Remote transcription is on, so dictations use the steps and vocabulary on that Hex. Changes here, Edit with agent, and hex settings on this Mac change them there", "A transcrição remota está ativada, então os ditados usam as etapas e o vocabulário desse Hex. As alterações aqui, Editar com agente e hex settings neste Mac as alteram lá"),
    ("Its Output steps aren't copied yet, so a fallback here uses this Mac's own", "As etapas de Saída dele ainda não foram copiadas, então o modo reserva aqui usa as deste Mac"),
    ("When it can't be reached, this Mac uses a copy of its Output steps and vocabulary", "Quando ele não está acessível, este Mac usa uma cópia das etapas de Saída e do vocabulário dele"),
    ("It transcribes with {theirs}, this Mac with {ours}. Choose the same model in General for matching text", "Ele transcreve com {theirs}, este Mac com {ours}. Escolha o mesmo modelo em Geral para obter o mesmo texto"),
    ("Transcribe again in transforms", "Transcrever de novo nas transformações"),
    ("Your transformations can hear a dictation again with another speech model", "Suas transformações podem ouvir um ditado de novo com outro modelo de voz"),
    ("Transcribe for your iPhone or another Mac", "Transcrever para o seu iPhone ou outro Mac"),
    ("Too many edits waiting to reach the other Hex; try again", "Há edições demais esperando para chegar ao outro Hex; tente novamente"),
    ("Sending edits stopped", "O envio das edições parou"),
    ("Remote transcription is off, so the edit wasn't sent", "A transcrição remota está desativada, então a edição não foi enviada"),
    ("{machine} refused the edit: {message}", "{machine} recusou a edição: {message}"),
];

/// Not yet reviewed by a native speaker; terms follow `super::hungarian`.
#[rustfmt::skip]
pub(super) const HUNGARIAN: &[(&str, &str)] = &[
    ("Remote transcription", "Távoli átírás"),
    ("Send dictations, meeting audio, and Annotate narration to another Hex, which transcribes them and runs its Output steps. When it can't be reached, this Mac does", "A diktálásokat, a megbeszélések hangját és az Annotálás narrációját egy másik Hexnek küldi, amely átírja őket és lefuttatja a Kimenet lépéseit. Ha nem érhető el, ez a Mac végzi"),
    ("Connecting stopped.", "A kapcsolódás megszakadt."),
    ("Disconnect", "Leválasztás"),
    ("Sends to {}", "Ide küld: {}"),
    ("Address or pair link", "Cím vagy párosítási hivatkozás"),
    ("Code", "Kód"),
    ("Connect", "Kapcsolódás"),
    ("On the other Hex, open Labs and choose Pair a device", "A másik Hexen nyissa meg a Labort, és válassza az Eszköz párosítása lehetőséget"),
    ("Connect to", "Kapcsolódás ehhez"),
    ("Enter the other Hex's address and code.", "Adja meg a másik Hex címét és kódját."),
    ("Pair a device", "Eszköz párosítása"),
    ("Transcribe for other devices", "Átírás más eszközöknek"),
    ("Hex on your iPhone or another Mac sends its dictations here, and this Hex transcribes them and runs its Output steps", "Az iPhone-ján vagy egy másik Macen futó Hex ide küldi a diktálásait, ez a Hex pedig átírja őket és lefuttatja a Kimenet lépéseit"),
    ("Revoke", "Visszavonás"),
    ("Remote devices", "Távoli eszközök"),
    ("Address {}", "Cím: {}"),
    ("Code {}", "Kód: {}"),
    ("Scan with Hex on your iPhone, or enter both on the other Mac. The code works once, for five minutes.", "Olvassa be az iPhone-ján futó Hexszel, vagy adja meg mindkettőt a másik Macen. A kód egyszer használható, öt percig."),
    ("Restart Hex to start serving devices.", "Indítsa újra a Hexet az eszközök kiszolgálásához."),
    ("Editing {}'s steps", "{} lépéseinek szerkesztése"),
    ("Remote transcription is on, so dictations use the steps and vocabulary on that Hex. Changes here, Edit with agent, and hex settings on this Mac change them there", "A távoli átírás be van kapcsolva, így a diktálások annak a Hexnek a lépéseit és szókincsét használják. Az itteni módosítások, a Szerkesztés ügynökkel és a hex settings ezen a Macen ott módosítják őket"),
    ("Its Output steps aren't copied yet, so a fallback here uses this Mac's own", "A Kimenet lépései még nincsenek átmásolva, ezért egy itteni tartalék ennek a Macnek a sajátjait használja"),
    ("When it can't be reached, this Mac uses a copy of its Output steps and vocabulary", "Ha nem érhető el, ez a Mac a Kimenet lépéseinek és a szókincsének másolatát használja"),
    ("It transcribes with {theirs}, this Mac with {ours}. Choose the same model in General for matching text", "Az a(z) {theirs} modellel ír át, ez a Mac a(z) {ours} modellel. Azonos szöveghez válassza ugyanazt a modellt az Általános részben"),
    ("Transcribe again in transforms", "Újraátírás az átalakításokban"),
    ("Your transformations can hear a dictation again with another speech model", "Az átalakításai egy másik beszédmodellel újra meghallgathatnak egy diktálást"),
    ("Transcribe for your iPhone or another Mac", "Átírás az iPhone-jának vagy egy másik Macnek"),
    ("Too many edits waiting to reach the other Hex; try again", "Túl sok módosítás vár arra, hogy eljusson a másik Hexhez; próbálja újra"),
    ("Sending edits stopped", "A módosítások küldése leállt"),
    ("Remote transcription is off, so the edit wasn't sent", "A távoli átírás ki van kapcsolva, ezért a módosítás nem lett elküldve"),
    ("{machine} refused the edit: {message}", "{machine} elutasította a módosítást: {message}"),
];
