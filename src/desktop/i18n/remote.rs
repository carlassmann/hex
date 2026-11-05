//! Remote transcription and paired devices' translated strings (this fork's
//! own), kept beside [`super::TRANSLATIONS`] so the main table stays under
//! the source cap.

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
        // No Hungarian strings for the fork's own yet: they read in English.
        Language::Hungarian => &[],
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
];
