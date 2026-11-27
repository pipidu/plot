//! 简体中文和英语以外的长说明。状态栏的带参数句子在 `sciplot_core::lang_formats`。

use sciplot_core::Language;

pub fn formula(language: Language) -> &'static str {
    match language {
        Language::ZhHant => FORMULA_HANT,
        Language::De => FORMULA_DE,
        Language::Fr => FORMULA_FR,
        Language::Ru => FORMULA_RU,
        Language::Es => FORMULA_ES,
        Language::Cs => FORMULA_CS,
        Language::PtBr => FORMULA_PT,
        Language::Zh | Language::En => "",
    }
}

pub fn script(language: Language) -> &'static str {
    match language {
        Language::ZhHant => SCRIPT_HANT,
        Language::De => SCRIPT_DE,
        Language::Fr => SCRIPT_FR,
        Language::Ru => SCRIPT_RU,
        Language::Es => SCRIPT_ES,
        Language::Cs => SCRIPT_CS,
        Language::PtBr => SCRIPT_PT,
        Language::Zh | Language::En => "",
    }
}

pub fn about(language: Language) -> &'static str {
    match language {
        Language::ZhHant => ABOUT_HANT,
        Language::De => ABOUT_DE,
        Language::Fr => ABOUT_FR,
        Language::Ru => ABOUT_RU,
        Language::Es => ABOUT_ES,
        Language::Cs => ABOUT_CS,
        Language::PtBr => ABOUT_PT,
        Language::Zh | Language::En => "",
    }
}

pub fn ai(language: Language) -> &'static str {
    match language {
        Language::ZhHant => AI_HANT,
        Language::De => AI_DE,
        Language::Fr => AI_FR,
        Language::Ru => AI_RU,
        Language::Es => AI_ES,
        Language::Cs => AI_CS,
        Language::PtBr => AI_PT,
        Language::Zh | Language::En => "",
    }
}

const FORMULA_HANT: &str = "\
運算式規則：用普通算式寫欄公式和擬合公式。求值器是 SciPlot 自己的運算式引擎，所有數都是小數，不是 evalexpr。

運算：+  -  *  /  %  ^（次方）和括號。
1/2 等於 0.5，不會截成整數。1 和 1.0 相等。

函數：sin cos tan asin acos atan atan2 sinh cosh tanh
      exp ln log（以 10 為底）log10 log2 sqrt abs
      floor ceil round min max hypot pow cbrt sign

常數：pi、e。函數曲線的自變數是 x。參數曲線用 t。極座標用 theta、t 或 x 表示弧度。曲面公式用 x 和 y。

重算依欄之間的依賴排一次序，每個公式只算一遍，不迭代到不動點。右邊的欄可以讀到左邊已經算完的儲存格公式。有環時按欄號、列號各算一次，並記一條警告。套用欄公式不會清掉這一欄已經有的儲存格公式。表格上方的公式列直接改目前格：以 = 開頭的是儲存格公式，否則寫入數值並清掉這一格自己的公式。

欄公式還可以用 col(\"欄名\")、col(\"A\")、col(1)、cell(\"A\", 列號)、i 或 row，以及 tablecol(\"表\",\"欄\")。

if()、?: 和縮排的 if / elif / else 只計算選中的一邊。以 python: 開頭的欄公式交給程式內的直譯器逐列計算。

「執行筆記」是逐行公式，不能用 col()。先重算表格再執行筆記時，col()、cell() 和 tablecol() 不會讀到上一張表的最後一列，而是報錯。「執行腳本」執行程式內的直譯器。這不是 CPython。
";

const FORMULA_DE: &str = "\
Ausdrücke: Spaltenformeln und Fit-Formeln sind gewöhnliche Rechnungen. Der Auswerter ist SciPlots eigene Ausdrucksmaschine. Jede Zahl ist eine Dezimalzahl, nicht evalexpr.

Operatoren: +  -  *  /  %  ^ (Potenz) und Klammern.
1/2 ist 0,5 und wird nicht abgeschnitten. 1 ist gleich 1,0.

Funktionen: sin cos tan asin acos atan atan2 sinh cosh tanh
            exp ln log (Basis 10) log10 log2 sqrt abs
            floor ceil round min max hypot pow cbrt sign

Konstanten: pi und e. Funktionskurven benutzen x. Eine Parameterkurve benutzt t. Polar benutzt theta, t oder x im Bogenmaß. Flächen benutzen x und y.

Die Neuberechnung ordnet Formeln einmal nach ihren Spaltenabhängigkeiten. Jede Formel wird einmal ausgewertet, nicht bis zu einem Fixpunkt. Eine Spalte rechts kann Zellformeln links lesen, die gerade berechnet wurden. Ein Zyklus wird einmal, nach Spalte und Zeile, ausgewertet und als Warnung notiert. Eine Spaltenformel löscht vorhandene Zellformeln dieser Spalte nicht. Die Formelleiste über der Tabelle ändert die aktuelle Zelle: Text mit = ist eine Zellformel, sonst wird der Wert geschrieben und die Formel dieser Zelle gelöscht.

Spaltenformeln können col(\"Name\"), col(\"A\"), col(1), cell(\"A\", Zeile), i oder row und tablecol(\"Tabelle\",\"Spalte\") verwenden.

if(), ?: und eingerücktes if / elif / else werten nur die gewählte Seite aus. Eine Spaltenformel mit python: wird zeilenweise vom eingebauten Interpreter berechnet.

Notiz ausführen ist eine Formel pro Zeile und kann col() nicht verwenden. Nach einer Tabellenberechnung lesen col(), cell() und tablecol() in einer Notiz nicht die letzte Zeile der vorigen Tabelle, sondern melden einen Fehler. Skript ausführen startet den eingebauten Interpreter. Das ist nicht CPython.
";

const FORMULA_FR: &str = "\
Expressions : les formules de colonne et d'ajustement sont des calculs ordinaires. L'évaluateur est le moteur d'expressions de SciPlot. Chaque nombre est un nombre décimal, pas evalexpr.

Opérateurs : +  -  *  /  %  ^ (puissance) et parenthèses.
1/2 vaut 0,5 et n'est pas tronqué. 1 est égal à 1,0.

Fonctions : sin cos tan asin acos atan atan2 sinh cosh tanh
            exp ln log (base 10) log10 log2 sqrt abs
            floor ceil round min max hypot pow cbrt sign

Constantes : pi et e. Une courbe de fonction utilise x. Une courbe paramétrique utilise t. Le polaire utilise theta, t ou x en radians. Une surface utilise x et y.

Le recalcul ordonne les formules une fois selon les dépendances des colonnes. Chaque formule est évaluée une fois, sans itérer jusqu'à un point fixe. Une colonne à droite peut lire les formules de cellule déjà calculées à gauche. Un cycle est évalué une fois, par colonne puis par ligne, et noté comme avertissement. Appliquer une formule de colonne ne supprime pas les formules de cellule déjà présentes. La barre de formule au-dessus du tableau modifie la cellule courante : un texte qui commence par = est une formule de cellule ; sinon la valeur est écrite et la formule de cette cellule est effacée.

Les formules de colonne peuvent utiliser col(\"nom\"), col(\"A\"), col(1), cell(\"A\", ligne), i ou row, et tablecol(\"table\",\"colonne\").

if(), ?: et if / elif / else indenté n'évaluent que le côté choisi. Une formule qui commence par python: est calculée ligne par ligne par l'interpréteur intégré.

Exécuter la note est une formule par ligne et ne peut pas utiliser col(). Après un recalcul de tableau, col(), cell() et tablecol() dans une note ne lisent pas la dernière ligne du tableau précédent : ils signalent une erreur. Exécuter le script lance l'interpréteur intégré. Ce n'est pas CPython.
";

const FORMULA_RU: &str = "\
Выражения: формулы столбцов и подгонки — обычные вычисления. Вычислитель — собственный движок выражений SciPlot. Каждое число дробное, это не evalexpr.

Операторы: +  -  *  /  %  ^ (степень) и скобки.
1/2 равно 0,5 и не обрезается. 1 равно 1,0.

Функции: sin cos tan asin acos atan atan2 sinh cosh tanh
          exp ln log (основание 10) log10 log2 sqrt abs
          floor ceil round min max hypot pow cbrt sign

Константы: pi и e. Кривая функции использует x. Параметрическая кривая использует t. Полярная — theta, t или x в радианах. Поверхность использует x и y.

Пересчёт один раз упорядочивает формулы по зависимостям столбцов. Каждая формула считается один раз, без итераций до неподвижной точки. Столбец справа может прочитать уже посчитанные формулы ячеек слева. Цикл считается один раз, по столбцу и строке, и записывается как предупреждение. Формула столбца не удаляет уже существующие формулы ячеек. Строка формул над таблицей меняет текущую ячейку: текст с = — формула ячейки, иначе записывается значение и формула этой ячейки очищается.

В формуле столбца доступны col(\"имя\"), col(\"A\"), col(1), cell(\"A\", строка), i или row и tablecol(\"таблица\",\"столбец\").

if(), ?: и if / elif / else с отступом считают только выбранную сторону. Формула с префиксом python: считается построчно встроенным интерпретатором.

Выполнение заметки — формула на строку, col() там нельзя. После пересчёта таблицы col(), cell() и tablecol() в заметке не читают последнюю строку предыдущей таблицы, а сообщают ошибку. Выполнение скрипта запускает встроенный интерпретатор. Это не CPython.
";

const FORMULA_ES: &str = "\
Expresiones: las fórmulas de columna y de ajuste son cálculos ordinarios. El evaluador es el motor de expresiones de SciPlot. Cada número es decimal, no evalexpr.

Operadores: +  -  *  /  %  ^ (potencia) y paréntesis.
1/2 es 0,5 y no se trunca. 1 es igual a 1,0.

Funciones: sin cos tan asin acos atan atan2 sinh cosh tanh
           exp ln log (base 10) log10 log2 sqrt abs
           floor ceil round min max hypot pow cbrt sign

Constantes: pi y e. Una curva de función usa x. Una curva paramétrica usa t. La polar usa theta, t o x en radianes. Una superficie usa x e y.

El recálculo ordena las fórmulas una vez según las dependencias de las columnas. Cada fórmula se evalúa una vez, sin iterar hasta un punto fijo. Una columna a la derecha puede leer fórmulas de celda ya calculadas a la izquierda. Un ciclo se evalúa una vez, por columna y fila, y se anota como aviso. Aplicar una fórmula de columna no borra las fórmulas de celda que ya existen. La barra de fórmulas sobre la tabla edita la celda actual: un texto que empieza por = es una fórmula de celda; si no, se escribe el valor y se borra la fórmula de esa celda.

Las fórmulas de columna pueden usar col(\"nombre\"), col(\"A\"), col(1), cell(\"A\", fila), i o row, y tablecol(\"tabla\",\"columna\").

if(), ?: y un if / elif / else con sangría evalúan solo el lado elegido. Una fórmula que empieza por python: se calcula fila a fila con el intérprete integrado.

Ejecutar la nota es una fórmula por línea y no puede usar col(). Tras recalcular una tabla, col(), cell() y tablecol() en una nota no leen la última fila de la tabla anterior: informan un error. Ejecutar el script lanza el intérprete integrado. No es CPython.
";

const FORMULA_CS: &str = "\
Výrazy: vzorce sloupců a fitování jsou běžné výpočty. Vyhodnocovač je vlastní stroj výrazů SciPlot. Každé číslo je desetinné, není to evalexpr.

Operátory: +  -  *  /  %  ^ (mocnina) a závorky.
1/2 je 0,5 a neoseká se. 1 se rovná 1,0.

Funkce: sin cos tan asin acos atan atan2 sinh cosh tanh
        exp ln log (základ 10) log10 log2 sqrt abs
        floor ceil round min max hypot pow cbrt sign

Konstanty: pi a e. Funkční křivka používá x. Parametrická křivka používá t. Polární používá theta, t nebo x v radiánech. Plocha používá x a y.

Přepočet jednou seřadí vzorce podle závislostí sloupců. Každý vzorec se vyhodnotí jednou, bez iterace k pevnému bodu. Sloupec vpravo může číst právě spočítané vzorce buněk vlevo. Cyklus se vyhodnotí jednou, podle sloupce a řádku, a zapíše se jako varování. Vzorec sloupce nesmaže už existující vzorce buněk. Řádek vzorců nad tabulkou mění aktuální buňku: text začínající = je vzorec buňky, jinak se zapíše hodnota a vzorec této buňky se smaže.

Vzorec sloupce může použít col(\"jméno\"), col(\"A\"), col(1), cell(\"A\", řádek), i nebo row a tablecol(\"tabulka\",\"sloupec\").

if(), ?: a odsazené if / elif / else vyhodnotí jen zvolenou stranu. Vzorec začínající python: počítá vestavěný interpret po řádcích.

Spuštění poznámky je vzorec na řádek a nemůže použít col(). Po přepočtu tabulky col(), cell() a tablecol() v poznámce nečtou poslední řádek předchozí tabulky, ale ohlásí chybu. Spuštění skriptu spustí vestavěný interpret. Není to CPython.
";

const FORMULA_PT: &str = "\
Expressões: fórmulas de coluna e de ajuste são contas comuns. O avaliador é o motor de expressões do SciPlot. Todo número é decimal, não é evalexpr.

Operadores: +  -  *  /  %  ^ (potência) e parênteses.
1/2 é 0,5 e não é truncado. 1 é igual a 1,0.

Funções: sin cos tan asin acos atan atan2 sinh cosh tanh
         exp ln log (base 10) log10 log2 sqrt abs
         floor ceil round min max hypot pow cbrt sign

Constantes: pi e e. Uma curva de função usa x. Uma curva paramétrica usa t. A polar usa theta, t ou x em radianos. Uma superfície usa x e y.

O recálculo ordena as fórmulas uma vez pelas dependências das colunas. Cada fórmula é avaliada uma vez, sem iterar até um ponto fixo. Uma coluna à direita pode ler fórmulas de célula já calculadas à esquerda. Um ciclo é avaliado uma vez, por coluna e linha, e registrado como aviso. Aplicar uma fórmula de coluna não apaga as fórmulas de célula que já existem. A barra de fórmula acima da tabela edita a célula atual: texto que começa com = é uma fórmula de célula; caso contrário o valor é gravado e a fórmula dessa célula é apagada.

Fórmulas de coluna podem usar col(\"nome\"), col(\"A\"), col(1), cell(\"A\", linha), i ou row, e tablecol(\"tabela\",\"coluna\").

if(), ?: e if / elif / else indentado avaliam só o lado escolhido. Uma fórmula que começa com python: é calculada linha a linha pelo interpretador embutido.

Executar a nota é uma fórmula por linha e não pode usar col(). Depois de recalcular uma tabela, col(), cell() e tablecol() numa nota não leem a última linha da tabela anterior: eles informam um erro. Executar o script inicia o interpretador embutido. Não é CPython.
";

const SCRIPT_HANT: &str = "\
腳本選單和筆記裡的「執行腳本」使用程式內的直譯器，不是 CPython。\n\
沒有嵌入 CPython：Windows 發行包在 Linux 上用 mingw 交叉編譯，PyO3 需要 libpython；再為三個平台帶上標準庫既放不進發行包，也會要求使用者另外安裝 Python。RustPython 的工具鏈高於本倉庫宣告的 1.81。\n\
可以寫 if/elif/else、for、while、def、清單、字典、類別、try/except，以及 table()/graph()/newTable()/plot()/Fit。\n\
import math、statistics、random、json 和 from sciplot import * 會裝上這些名字。不認識的 import 會報錯。\n\
腳本裡的 / 是真除法，// 是向下取整。欄公式的 1/2 也是 0.5。\n\
newTable(名字, 欄數, 列數)。setCell 和 setColName 的序號從 1 開始。\n\
常用方法還有欄名、公式、importASCII、exportASCII、座標軸標題、saveProject，以及 tableList/graphList。";

const SCRIPT_DE: &str = "\
Das Menü Skript und Skript ausführen in einer Notiz verwenden den eingebauten Interpreter, nicht CPython.\n\
CPython ist nicht eingebettet. Das Windows-Paket wird unter Linux mit mingw gekreuzt übersetzt, und PyO3 braucht libpython. Eine Standardbibliothek für drei Plattformen passt nicht in die Ausgabe, und dynamisches Linken würde verlangen, dass Nutzer Python selbst installieren. Die Werkzeugkette von RustPython ist neuer als das hier erklärte 1.81.\n\
Sie können if/elif/else, for, while, def, Listen, Dictionaries, Klassen, try/except und table()/graph()/newTable()/plot()/Fit schreiben.\n\
import math, statistics, random, json und from sciplot import * stellen diese Namen bereit. Ein unbekanntes import ist ein Fehler.\n\
Im Skript ist / echte Division und // rundet ab. 1/2 in einer Spaltenformel ist ebenfalls 0,5.\n\
newTable(Name, Spalten, Zeilen). setCell und setColName zählen ab 1.\n\
Weitere Methoden: Spaltennamen, Formeln, importASCII, exportASCII, Achsentitel, saveProject sowie tableList/graphList.";

const SCRIPT_FR: &str = "\
Le menu Script et Exécuter le script dans une note utilisent l'interpréteur intégré, pas CPython.\n\
CPython n'est pas intégré. Le paquet Windows est compilé de façon croisée avec mingw, et PyO3 a besoin de libpython. Une bibliothèque standard pour trois plateformes ne tient pas dans la distribution, et une liaison dynamique obligerait l'utilisateur à installer Python. La chaîne d'outils de RustPython est plus récente que le 1.81 déclaré.\n\
Vous pouvez écrire if/elif/else, for, while, def, des listes, des dictionnaires, des classes, try/except, et table()/graph()/newTable()/plot()/Fit.\n\
import math, statistics, random, json et from sciplot import * installent ces noms. Un import inconnu est une erreur.\n\
Dans un script, / est une vraie division et // arrondit vers le bas. 1/2 dans une formule de colonne vaut aussi 0,5.\n\
newTable(nom, colonnes, lignes). setCell et setColName commencent à 1.\n\
Autres méthodes courantes : noms de colonnes, formules, importASCII, exportASCII, titres d'axes, saveProject, et tableList/graphList.";

const SCRIPT_RU: &str = "\
Меню «Скрипт» и выполнение скрипта в заметке используют встроенный интерпретатор, не CPython.\n\
CPython не встроен. Пакет Windows кросс-компилируется mingw, а PyO3 нужен libpython. Стандартная библиотека для трёх платформ не помещается в выпуск, а динамическая связь заставила бы пользователя ставить Python отдельно. Инструменты RustPython новее объявленной здесь 1.81.\n\
Можно писать if/elif/else, for, while, def, списки, словари, классы, try/except и table()/graph()/newTable()/plot()/Fit.\n\
import math, statistics, random, json и from sciplot import * создают эти имена. Неизвестный import — ошибка.\n\
В скрипте / — настоящее деление, // — деление с округлением вниз. 1/2 в формуле столбца тоже 0,5.\n\
newTable(имя, столбцы, строки). setCell и setColName нумеруются с 1.\n\
Ещё есть имена столбцов, формулы, importASCII, exportASCII, заголовки осей, saveProject и tableList/graphList.";

const SCRIPT_ES: &str = "\
El menú Script y Ejecutar script en una nota usan el intérprete integrado, no CPython.\n\
CPython no está integrado. El paquete de Windows se compila de forma cruzada con mingw, y PyO3 necesita libpython. Una biblioteca estándar para tres plataformas no cabe en la distribución, y un enlace dinámico obligaría a instalar Python. La cadena de RustPython es más nueva que el 1.81 declarado.\n\
Puede escribir if/elif/else, for, while, def, listas, diccionarios, clases, try/except y table()/graph()/newTable()/plot()/Fit.\n\
import math, statistics, random, json y from sciplot import * instalan esos nombres. Un import desconocido es un error.\n\
En un script, / es división real y // redondea hacia abajo. 1/2 en una fórmula de columna también es 0,5.\n\
newTable(nombre, columnas, filas). setCell y setColName empiezan en 1.\n\
Otros métodos: nombres de columna, fórmulas, importASCII, exportASCII, títulos de ejes, saveProject y tableList/graphList.";

const SCRIPT_CS: &str = "\
Nabídka Skript a spuštění skriptu v poznámce používají vestavěný interpret, ne CPython.\n\
CPython není vestavěný. Balíček pro Windows se křížově překládá mingw a PyO3 potřebuje libpython. Standardní knihovna pro tři platformy se do vydání nevejde a dynamické linkování by nutilo uživatele instalovat Python. Nástroje RustPython jsou novější než zde uvedené 1.81.\n\
Lze psát if/elif/else, for, while, def, seznamy, slovníky, třídy, try/except a table()/graph()/newTable()/plot()/Fit.\n\
import math, statistics, random, json a from sciplot import * tato jména zavedou. Neznámý import je chyba.\n\
Ve skriptu je / skutečné dělení a // zaokrouhluje dolů. 1/2 ve vzorci sloupce je také 0,5.\n\
newTable(jméno, sloupce, řádky). setCell a setColName se číslují od 1.\n\
Další metody: jména sloupců, vzorce, importASCII, exportASCII, názvy os, saveProject a tableList/graphList.";

const SCRIPT_PT: &str = "\
O menu Script e Executar script numa nota usam o interpretador embutido, não CPython.\n\
O CPython não está embutido. O pacote Windows é compilado de forma cruzada com mingw, e o PyO3 precisa de libpython. Uma biblioteca padrão para três plataformas não cabe no pacote, e a ligação dinâmica obrigaria o usuário a instalar o Python. A cadeia do RustPython é mais nova que o 1.81 declarado.\n\
Dá para escrever if/elif/else, for, while, def, listas, dicionários, classes, try/except e table()/graph()/newTable()/plot()/Fit.\n\
import math, statistics, random, json e from sciplot import * instalam esses nomes. Um import desconhecido é um erro.\n\
Num script, / é divisão real e // arredonda para baixo. 1/2 numa fórmula de coluna também é 0,5.\n\
newTable(nome, colunas, linhas). setCell e setColName começam em 1.\n\
Outros métodos: nomes de coluna, fórmulas, importASCII, exportASCII, títulos dos eixos, saveProject e tableList/graphList.";

const ABOUT_HANT: &str = "\
科學繪圖軟體。用 Rust 和 egui 編寫，界面對齊 SciDAVis。

表格、矩陣、多種二維圖、三維曲面、擬合、微積分、插值、平滑、FFT，以及曲線求交點。

專案可以存成 .splot 或壓縮的 .splot.gz。SciPlot 自己的 .sciprj 可以往返；官方 SciDAVis 專案也能開啟，儲存時另存為 .splot，不會覆蓋原檔。Origin 可以讀入數值、文字和日期欄，不能寫回去。圖形可以匯出 PNG、SVG 和 PDF。透明背景對 PNG、XPM 和 SVG 生效。EPS 和 PDF 裡的影像標註帶上像素。

發布頁有 Windows、Linux，以及未簽名、未公證的 macOS 程式（Apple Silicon，和未在 Intel 硬體上執行過的 x86_64 套件）。沒有 Apple Developer 憑證。
";

const ABOUT_DE: &str = "\
Wissenschaftliche Diagramme, geschrieben in Rust und egui, mit einem Aufbau nahe an SciDAVis.

Tabellen, Matrizen, viele zweidimensionale Diagramme, dreidimensionale Flächen, Fits, Analysis, Interpolation, Glättung, FFT und Schnittpunkte.

Ein Projekt wird als .splot oder komprimiertes .splot.gz gespeichert. SciPlots eigenes .sciprj lässt sich hin und zurück lesen. Offizielle SciDAVis-Projekte lassen sich öffnen; Speichern schreibt eine .splot-Datei und überschreibt die Originaldatei nicht. Origin-Dateien können gelesen werden, einschließlich Text- und Datumsspalten, und können nicht zurückgeschrieben werden. Diagramme gehen nach PNG, SVG und PDF. Ein transparenter Hintergrund gilt für PNG, XPM und SVG. Bildanmerkungen in EPS und PDF behalten ihre Pixel.

Die Veröffentlichung enthält Windows, Linux und ein unsigniertes, nicht notarisiertes macOS-Programm (Apple Silicon und ein x86_64-Paket, das nicht auf Intel-Hardware lief). Es gibt kein Apple-Developer-Zertifikat.
";

const ABOUT_FR: &str = "\
Tracé scientifique, écrit en Rust et egui, avec une disposition proche de SciDAVis.

Tableaux, matrices, de nombreux tracés bidimensionnels, surfaces tridimensionnelles, ajustements, analyse, interpolation, lissage, FFT et intersections.

Un projet s'enregistre en .splot ou .splot.gz compressé. Le .sciprj propre à SciPlot fait l'aller-retour. Les projets officiels SciDAVis s'ouvrent ; l'enregistrement écrit un .splot et n'écrase pas l'original. Les fichiers Origin se lisent, y compris les colonnes de texte et de date, et ne se réécrivent pas. Les graphiques s'exportent en PNG, SVG et PDF. Un fond transparent s'applique au PNG, au XPM et au SVG. Les annotations image en EPS et PDF gardent leurs pixels.

La publication contient Windows, Linux et un programme macOS non signé et non notarisé (Apple Silicon, plus un paquet x86_64 qui n'a pas tourné sur du matériel Intel). Il n'y a pas de certificat Apple Developer.
";

const ABOUT_RU: &str = "\
Научные графики, написанные на Rust и egui, с расположением, близким к SciDAVis.

Таблицы, матрицы, много двумерных графиков, трёхмерные поверхности, подгонка, анализ, интерполяция, сглаживание, FFT и пересечения.

Проект сохраняется как .splot или сжатый .splot.gz. Собственный .sciprj SciPlot читается в обе стороны. Официальные проекты SciDAVis открываются; сохранение пишет .splot и не перезаписывает исходный файл. Файлы Origin читаются, включая текстовые столбцы и даты, и не записываются обратно. Графики экспортируются в PNG, SVG и PDF. Прозрачный фон действует для PNG, XPM и SVG. Изображения в EPS и PDF сохраняют пиксели.

Выпуск содержит Windows, Linux и неподписанную, не нотаризованную программу macOS (Apple Silicon и пакет x86_64, который не запускался на оборудовании Intel). Сертификата Apple Developer нет.
";

const ABOUT_ES: &str = "\
Gráficos científicos, escritos en Rust y egui, con una disposición cercana a SciDAVis.

Tablas, matrices, muchos gráficos bidimensionales, superficies tridimensionales, ajustes, análisis, interpolación, suavizado, FFT e intersecciones.

Un proyecto se guarda como .splot o .splot.gz comprimido. El .sciprj propio de SciPlot se puede abrir y guardar. Los proyectos oficiales de SciDAVis se abren; guardar escribe un .splot y no sobrescribe el original. Los archivos Origin se leen, incluidas columnas de texto y fecha, y no se pueden volver a escribir. Los gráficos se exportan a PNG, SVG y PDF. Un fondo transparente se aplica a PNG, XPM y SVG. Las anotaciones de imagen en EPS y PDF conservan los píxeles.

La publicación incluye Windows, Linux y un programa macOS sin firmar y sin notarizar (Apple Silicon y un paquete x86_64 que no se ejecutó en hardware Intel). No hay certificado de Apple Developer.
";

const ABOUT_CS: &str = "\
Vědecké grafy, napsané v Rustu a egui, s rozložením blízkým SciDAVis.

Tabulky, matice, mnoho dvourozměrných grafů, trojrozměrné plochy, fity, analýza, interpolace, vyhlazení, FFT a průsečíky.

Projekt se ukládá jako .splot nebo komprimovaný .splot.gz. Vlastní .sciprj SciPlot lze číst oběma směry. Oficiální projekty SciDAVis se otevírají; uložení zapíše .splot a původní soubor nepřepíše. Soubory Origin se čtou, včetně textových a datumových sloupců, a nelze je zapsat zpět. Grafy se exportují do PNG, SVG a PDF. Průhledné pozadí platí pro PNG, XPM a SVG. Obrazové popisky v EPS a PDF si ponechají pixely.

Vydání obsahuje Windows, Linux a nepodepsaný, nenotářsky ověřený program macOS (Apple Silicon a balíček x86_64, který neběžel na hardwaru Intel). Certifikát Apple Developer není.
";

const ABOUT_PT: &str = "\
Gráficos científicos, escritos em Rust e egui, com um arranjo próximo do SciDAVis.

Tabelas, matrizes, muitos gráficos bidimensionais, superfícies tridimensionais, ajustes, análise, interpolação, suavização, FFT e interseções.

Um projeto é salvo como .splot ou .splot.gz comprimido. O .sciprj do próprio SciPlot vai e volta. Projetos oficiais do SciDAVis abrem; salvar grava um .splot e não substitui o original. Arquivos Origin são lidos, inclusive colunas de texto e data, e não podem ser gravados de volta. Gráficos exportam para PNG, SVG e PDF. Um fundo transparente vale para PNG, XPM e SVG. Anotações de imagem em EPS e PDF mantêm os pixels.

A publicação inclui Windows, Linux e um programa macOS sem assinatura e sem notarização (Apple Silicon e um pacote x86_64 que não rodou em hardware Intel). Não há certificado Apple Developer.
";

const AI_HANT: &str = "\
讓 AI 在 SciPlot 裡畫圖有兩種用法。

1. 視窗開著時，本機會在 127.0.0.1 上開一個 API，只接受帶令牌的請求。令牌和連接埠寫在主目錄的 .sciplot_api.json，結束時刪除。可以在「設定 → 預設樣式與字型」裡關掉，或改連接埠。AI 的改動記在復原裡。

2. 不開視窗時，用命令列：
   sciplot --mcp
   sciplot --mcp --project 實驗.splot
   視窗開著時，--mcp 會把呼叫轉給視窗；沒開時就在這個行程裡改專案。帶了 --project 才會在每次改動後寫回檔案。

Claude Desktop 或 Cursor 的設定：

{
  \"mcpServers\": {
    \"sciplot\": {
      \"command\": \"sciplot\",
      \"args\": [\"--mcp\"]
    }
  }
}

Windows 上把 command 寫成 sciplot.exe 的完整路徑。標準輸出只寫 MCP 協定，不要把日誌重定向到標準輸出。
";

const AI_DE: &str = "\
Eine KI kann in SciPlot auf zwei Arten zeichnen.

1. Bei offenem Fenster hört SciPlot auf 127.0.0.1 und nimmt nur Anfragen mit Token an. Token und Port stehen in .sciplot_api.json im Home-Verzeichnis und werden beim Beenden gelöscht. Abschalten oder den Port ändern unter Einstellungen → Standardstil und Schriften. Änderungen der KI lassen sich rückgängig machen.

2. Ohne Fenster:
   sciplot --mcp
   sciplot --mcp --project experiment.splot
   Ist ein Fenster offen, leitet --mcp die Aufrufe dorthin. Sonst bearbeitet es ein Projekt in diesem Prozess. Eine --project-Datei wird nach jeder Änderung zurückgeschrieben.

Konfiguration für Claude Desktop oder Cursor:

{
  \"mcpServers\": {
    \"sciplot\": {
      \"command\": \"sciplot\",
      \"args\": [\"--mcp\"]
    }
  }
}

Unter Windows ist command der volle Pfad zu sciplot.exe. Die Standardausgabe ist dem MCP-Protokoll vorbehalten; Logs dürfen nicht dorthin umgeleitet werden.
";

const AI_FR: &str = "\
Une IA peut tracer dans SciPlot de deux façons.

1. Fenêtre ouverte : SciPlot écoute sur 127.0.0.1 et n'accepte que les requêtes qui portent le jeton. Le jeton et le port sont écrits dans .sciplot_api.json du dossier personnel et supprimés à la fermeture. On le coupe, ou on change le port, dans Réglages → Style et polices par défaut. Les modifications de l'IA s'annulent.

2. Sans fenêtre :
   sciplot --mcp
   sciplot --mcp --project experience.splot
   Si une fenêtre est ouverte, --mcp lui transmet les appels. Sinon il modifie un projet dans ce processus. Un fichier --project est réécrit après chaque changement.

Configuration de Claude Desktop ou Cursor :

{
  \"mcpServers\": {
    \"sciplot\": {
      \"command\": \"sciplot\",
      \"args\": [\"--mcp\"]
    }
  }
}

Sous Windows, command est le chemin complet de sciplot.exe. La sortie standard est réservée au protocole MCP ; n'y redirigez pas les journaux.
";

const AI_RU: &str = "\
ИИ может строить графики в SciPlot двумя способами.

1. При открытом окне SciPlot слушает 127.0.0.1 и принимает только запросы с токеном. Токен и порт записываются в .sciplot_api.json в домашнем каталоге и удаляются при выходе. Выключить или сменить порт: Настройки → Стиль и шрифты по умолчанию. Изменения ИИ можно отменить.

2. Без окна:
   sciplot --mcp
   sciplot --mcp --project experiment.splot
   Если окно открыто, --mcp передаёт вызовы ему. Иначе проект меняется в этом процессе. Файл --project записывается после каждого изменения.

Настройка Claude Desktop или Cursor:

{
  \"mcpServers\": {
    \"sciplot\": {
      \"command\": \"sciplot\",
      \"args\": [\"--mcp\"]
    }
  }
}

В Windows command — полный путь к sciplot.exe. Стандартный вывод занят протоколом MCP; не перенаправляйте туда журнал.
";

const AI_ES: &str = "\
Una IA puede dibujar en SciPlot de dos maneras.

1. Con la ventana abierta, SciPlot escucha en 127.0.0.1 y solo acepta peticiones con el token. El token y el puerto se escriben en .sciplot_api.json del directorio personal y se borran al salir. Se apaga, o se cambia el puerto, en Ajustes → Estilo y fuentes predeterminados. Los cambios de la IA se pueden deshacer.

2. Sin ventana:
   sciplot --mcp
   sciplot --mcp --project experimento.splot
   Si hay una ventana abierta, --mcp le reenvía las llamadas. Si no, edita un proyecto en este proceso. Un archivo --project se vuelve a escribir tras cada cambio.

Configuración de Claude Desktop o Cursor:

{
  \"mcpServers\": {
    \"sciplot\": {
      \"command\": \"sciplot\",
      \"args\": [\"--mcp\"]
    }
  }
}

En Windows, command es la ruta completa de sciplot.exe. La salida estándar está reservada al protocolo MCP; no redirija allí el registro.
";

const AI_CS: &str = "\
Umělá inteligence může v SciPlot kreslit dvěma způsoby.

1. Při otevřeném okně SciPlot poslouchá na 127.0.0.1 a přijímá jen požadavky s tokenem. Token a port jsou v .sciplot_api.json v domovském adresáři a při ukončení se smažou. Vypnout nebo změnit port: Nastavení → Výchozí styl a písma. Změny umělé inteligence jdou vzít zpět.

2. Bez okna:
   sciplot --mcp
   sciplot --mcp --project experiment.splot
   Je-li okno otevřené, --mcp mu volání předá. Jinak upraví projekt v tomto procesu. Soubor --project se zapíše po každé změně.

Nastavení Claude Desktop nebo Cursor:

{
  \"mcpServers\": {
    \"sciplot\": {
      \"command\": \"sciplot\",
      \"args\": [\"--mcp\"]
    }
  }
}

Ve Windows je command celá cesta k sciplot.exe. Standardní výstup je vyhrazen protokolu MCP; nepřesměrovávejte tam protokol.
";

const AI_PT: &str = "\
Uma IA pode desenhar no SciPlot de dois jeitos.

1. Com a janela aberta, o SciPlot escuta em 127.0.0.1 e só aceita pedidos com o token. O token e a porta ficam em .sciplot_api.json no diretório pessoal e são apagados ao sair. Desligue, ou mude a porta, em Configurações → Estilo e fontes padrão. As mudanças da IA podem ser desfeitas.

2. Sem janela:
   sciplot --mcp
   sciplot --mcp --project experimento.splot
   Se uma janela estiver aberta, --mcp encaminha as chamadas para ela. Caso contrário, edita um projeto neste processo. Um arquivo --project é gravado de volta após cada mudança.

Configuração do Claude Desktop ou do Cursor:

{
  \"mcpServers\": {
    \"sciplot\": {
      \"command\": \"sciplot\",
      \"args\": [\"--mcp\"]
    }
  }
}

No Windows, command é o caminho completo de sciplot.exe. A saída padrão fica reservada ao protocolo MCP; não redirecione o registro para lá.
";
