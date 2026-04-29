//! Built-in translation tables.
//!
//! Hard-coded `HashMap` translations for the most common UI strings.
//! Additional locales fall back to the English table.

use std::collections::HashMap;

use crate::types::Locale;

// ---------------------------------------------------------------------------
// Public API
// ---------------------------------------------------------------------------

/// Return the built-in translation map for the given locale.
///
/// If the locale has no dedicated table, the English table is returned.
pub fn get_translations(locale: Locale) -> HashMap<String, String> {
    match locale {
        Locale::En => en_translations(),
        Locale::ZhCn => zh_cn_translations(),
        Locale::Ja => ja_translations(),
        Locale::Ko => ko_translations(),
        Locale::De => de_translations(),
        Locale::Fr => fr_translations(),
        Locale::Es => es_translations(),
        Locale::It => it_translations(),
        Locale::Pt => pt_translations(),
        Locale::Ru => ru_translations(),
        Locale::Ar => ar_translations(),
        Locale::Hi => hi_translations(),
        Locale::Th => th_translations(),
        Locale::Vi => vi_translations(),
        Locale::Pl => pl_translations(),
        Locale::Nl => nl_translations(),
        Locale::Tr => tr_translations(),
        Locale::ZhTw => zh_tw_translations(),
    }
}

// ---------------------------------------------------------------------------
// Translation tables
// ---------------------------------------------------------------------------

fn en_translations() -> HashMap<String, String> {
    let mut m = HashMap::new();
    m.insert("save".into(), "Save".into());
    m.insert("cancel".into(), "Cancel".into());
    m.insert("error".into(), "Error".into());
    m.insert("warning".into(), "Warning".into());
    m.insert("loading".into(), "Loading...".into());
    m.insert("yes".into(), "Yes".into());
    m.insert("no".into(), "No".into());
    m.insert("ok".into(), "OK".into());
    m.insert("retry".into(), "Retry".into());
    m.insert("close".into(), "Close".into());
    m.insert("settings".into(), "Settings".into());
    m.insert("help".into(), "Help".into());
    m.insert("about".into(), "About".into());
    m.insert("version".into(), "Version".into());
    m.insert("language".into(), "Language".into());
    m.insert("theme".into(), "Theme".into());
    m.insert("mode".into(), "Mode".into());
    m.insert("tool".into(), "Tool".into());
    m.insert("file".into(), "File".into());
    m.insert("folder".into(), "Folder".into());
    m.insert("search".into(), "Search".into());
    m.insert("edit".into(), "Edit".into());
    m.insert("delete".into(), "Delete".into());
    m.insert("create".into(), "Create".into());
    m.insert("update".into(), "Update".into());
    m.insert("run".into(), "Run".into());
    m.insert("stop".into(), "Stop".into());
    m.insert("start".into(), "Start".into());
    m.insert("pause".into(), "Pause".into());
    m.insert("resume".into(), "Resume".into());
    m.insert("reset".into(), "Reset".into());
    m.insert("clear".into(), "Clear".into());
    m.insert("refresh".into(), "Refresh".into());
    m.insert("welcome".into(), "Welcome, {{name}}!".into());
    m.insert("items_count".into(), "{{count}} items".into());
    m.insert("no_workspace".into(), "Please open a project folder first".into());
    m.insert("not_git_repo".into(), "Not a git repository".into());
    m.insert("extension_name".into(), "Roo Code".into());
    m.insert(
        "extension_description".into(),
        "A whole dev team of AI agents in your editor.".into(),
    );
    m
}

fn zh_cn_translations() -> HashMap<String, String> {
    let mut m = HashMap::new();
    m.insert("save".into(), "保存".into());
    m.insert("cancel".into(), "取消".into());
    m.insert("error".into(), "错误".into());
    m.insert("warning".into(), "警告".into());
    m.insert("loading".into(), "加载中...".into());
    m.insert("yes".into(), "是".into());
    m.insert("no".into(), "否".into());
    m.insert("ok".into(), "确定".into());
    m.insert("retry".into(), "重试".into());
    m.insert("close".into(), "关闭".into());
    m.insert("settings".into(), "设置".into());
    m.insert("help".into(), "帮助".into());
    m.insert("about".into(), "关于".into());
    m.insert("version".into(), "版本".into());
    m.insert("language".into(), "语言".into());
    m.insert("theme".into(), "主题".into());
    m.insert("mode".into(), "模式".into());
    m.insert("tool".into(), "工具".into());
    m.insert("file".into(), "文件".into());
    m.insert("folder".into(), "文件夹".into());
    m.insert("search".into(), "搜索".into());
    m.insert("edit".into(), "编辑".into());
    m.insert("delete".into(), "删除".into());
    m.insert("create".into(), "创建".into());
    m.insert("update".into(), "更新".into());
    m.insert("run".into(), "运行".into());
    m.insert("stop".into(), "停止".into());
    m.insert("start".into(), "开始".into());
    m.insert("pause".into(), "暂停".into());
    m.insert("resume".into(), "继续".into());
    m.insert("reset".into(), "重置".into());
    m.insert("clear".into(), "清除".into());
    m.insert("refresh".into(), "刷新".into());
    m.insert("welcome".into(), "欢迎，{{name}}！".into());
    m.insert("items_count".into(), "{{count}} 个项目".into());
    m.insert("no_workspace".into(), "请先打开项目文件夹".into());
    m.insert("not_git_repo".into(), "不是 Git 仓库".into());
    m.insert("extension_name".into(), "Roo Code".into());
    m.insert(
        "extension_description".into(),
        "您编辑器中的完整AI开发团队。".into(),
    );
    m
}

fn ja_translations() -> HashMap<String, String> {
    let mut m = HashMap::new();
    m.insert("save".into(), "保存".into());
    m.insert("cancel".into(), "キャンセル".into());
    m.insert("error".into(), "エラー".into());
    m.insert("warning".into(), "警告".into());
    m.insert("loading".into(), "読み込み中...".into());
    m.insert("yes".into(), "はい".into());
    m.insert("no".into(), "いいえ".into());
    m.insert("ok".into(), "OK".into());
    m.insert("settings".into(), "設定".into());
    m.insert("search".into(), "検索".into());
    m.insert("file".into(), "ファイル".into());
    m.insert("folder".into(), "フォルダ".into());
    m.insert("delete".into(), "削除".into());
    m.insert("create".into(), "作成".into());
    m.insert("run".into(), "実行".into());
    m.insert("stop".into(), "停止".into());
    m.insert("language".into(), "言語".into());
    m.insert("mode".into(), "モード".into());
    m
}

fn ko_translations() -> HashMap<String, String> {
    let mut m = HashMap::new();
    m.insert("save".into(), "저장".into());
    m.insert("cancel".into(), "취소".into());
    m.insert("error".into(), "오류".into());
    m.insert("warning".into(), "경고".into());
    m.insert("loading".into(), "로딩 중...".into());
    m.insert("yes".into(), "예".into());
    m.insert("no".into(), "아니요".into());
    m.insert("ok".into(), "확인".into());
    m.insert("settings".into(), "설정".into());
    m.insert("search".into(), "검색".into());
    m.insert("file".into(), "파일".into());
    m.insert("folder".into(), "폴더".into());
    m.insert("delete".into(), "삭제".into());
    m.insert("create".into(), "만들기".into());
    m.insert("language".into(), "언어".into());
    m.insert("mode".into(), "모드".into());
    m
}

fn de_translations() -> HashMap<String, String> {
    let mut m = HashMap::new();
    m.insert("save".into(), "Speichern".into());
    m.insert("cancel".into(), "Abbrechen".into());
    m.insert("error".into(), "Fehler".into());
    m.insert("warning".into(), "Warnung".into());
    m.insert("yes".into(), "Ja".into());
    m.insert("no".into(), "Nein".into());
    m.insert("ok".into(), "OK".into());
    m.insert("settings".into(), "Einstellungen".into());
    m.insert("search".into(), "Suchen".into());
    m.insert("file".into(), "Datei".into());
    m.insert("folder".into(), "Ordner".into());
    m.insert("delete".into(), "Löschen".into());
    m.insert("language".into(), "Sprache".into());
    m
}

fn fr_translations() -> HashMap<String, String> {
    let mut m = HashMap::new();
    m.insert("save".into(), "Enregistrer".into());
    m.insert("cancel".into(), "Annuler".into());
    m.insert("error".into(), "Erreur".into());
    m.insert("warning".into(), "Avertissement".into());
    m.insert("yes".into(), "Oui".into());
    m.insert("no".into(), "Non".into());
    m.insert("ok".into(), "OK".into());
    m.insert("settings".into(), "Paramètres".into());
    m.insert("search".into(), "Rechercher".into());
    m.insert("file".into(), "Fichier".into());
    m.insert("folder".into(), "Dossier".into());
    m.insert("delete".into(), "Supprimer".into());
    m.insert("language".into(), "Langue".into());
    m
}

fn es_translations() -> HashMap<String, String> {
    let mut m = HashMap::new();
    m.insert("save".into(), "Guardar".into());
    m.insert("cancel".into(), "Cancelar".into());
    m.insert("error".into(), "Error".into());
    m.insert("warning".into(), "Advertencia".into());
    m.insert("yes".into(), "Sí".into());
    m.insert("no".into(), "No".into());
    m.insert("ok".into(), "OK".into());
    m.insert("settings".into(), "Configuración".into());
    m.insert("search".into(), "Buscar".into());
    m.insert("file".into(), "Archivo".into());
    m.insert("folder".into(), "Carpeta".into());
    m.insert("delete".into(), "Eliminar".into());
    m.insert("language".into(), "Idioma".into());
    m
}

fn it_translations() -> HashMap<String, String> {
    let mut m = HashMap::new();
    m.insert("save".into(), "Salva".into());
    m.insert("cancel".into(), "Annulla".into());
    m.insert("error".into(), "Errore".into());
    m.insert("warning".into(), "Avviso".into());
    m.insert("yes".into(), "Sì".into());
    m.insert("no".into(), "No".into());
    m.insert("ok".into(), "OK".into());
    m.insert("settings".into(), "Impostazioni".into());
    m.insert("search".into(), "Cerca".into());
    m.insert("file".into(), "File".into());
    m.insert("folder".into(), "Cartella".into());
    m.insert("delete".into(), "Elimina".into());
    m.insert("language".into(), "Lingua".into());
    m
}

fn pt_translations() -> HashMap<String, String> {
    let mut m = HashMap::new();
    m.insert("save".into(), "Salvar".into());
    m.insert("cancel".into(), "Cancelar".into());
    m.insert("error".into(), "Erro".into());
    m.insert("warning".into(), "Aviso".into());
    m.insert("yes".into(), "Sim".into());
    m.insert("no".into(), "Não".into());
    m.insert("ok".into(), "OK".into());
    m.insert("settings".into(), "Configurações".into());
    m.insert("search".into(), "Pesquisar".into());
    m.insert("file".into(), "Arquivo".into());
    m.insert("folder".into(), "Pasta".into());
    m.insert("delete".into(), "Excluir".into());
    m.insert("language".into(), "Idioma".into());
    m
}

fn ru_translations() -> HashMap<String, String> {
    let mut m = HashMap::new();
    m.insert("save".into(), "Сохранить".into());
    m.insert("cancel".into(), "Отмена".into());
    m.insert("error".into(), "Ошибка".into());
    m.insert("warning".into(), "Предупреждение".into());
    m.insert("yes".into(), "Да".into());
    m.insert("no".into(), "Нет".into());
    m.insert("ok".into(), "OK".into());
    m.insert("settings".into(), "Настройки".into());
    m.insert("search".into(), "Поиск".into());
    m.insert("file".into(), "Файл".into());
    m.insert("folder".into(), "Папка".into());
    m.insert("delete".into(), "Удалить".into());
    m.insert("language".into(), "Язык".into());
    m
}

fn ar_translations() -> HashMap<String, String> {
    let mut m = HashMap::new();
    m.insert("save".into(), "حفظ".into());
    m.insert("cancel".into(), "إلغاء".into());
    m.insert("error".into(), "خطأ".into());
    m.insert("warning".into(), "تحذير".into());
    m.insert("yes".into(), "نعم".into());
    m.insert("no".into(), "لا".into());
    m.insert("ok".into(), "موافق".into());
    m.insert("settings".into(), "الإعدادات".into());
    m.insert("search".into(), "بحث".into());
    m.insert("file".into(), "ملف".into());
    m.insert("folder".into(), "مجلد".into());
    m.insert("delete".into(), "حذف".into());
    m.insert("language".into(), "اللغة".into());
    m
}

fn hi_translations() -> HashMap<String, String> {
    let mut m = HashMap::new();
    m.insert("save".into(), "सहेजें".into());
    m.insert("cancel".into(), "रद्द करें".into());
    m.insert("error".into(), "त्रुटि".into());
    m.insert("warning".into(), "चेतावनी".into());
    m.insert("yes".into(), "हाँ".into());
    m.insert("no".into(), "नहीं".into());
    m.insert("ok".into(), "ठीक है".into());
    m.insert("settings".into(), "सेटिंग्स".into());
    m.insert("search".into(), "खोजें".into());
    m.insert("file".into(), "फ़ाइल".into());
    m.insert("folder".into(), "फ़ोल्डर".into());
    m.insert("delete".into(), "हटाएँ".into());
    m.insert("language".into(), "भाषा".into());
    m
}

fn th_translations() -> HashMap<String, String> {
    let mut m = HashMap::new();
    m.insert("save".into(), "บันทึก".into());
    m.insert("cancel".into(), "ยกเลิก".into());
    m.insert("error".into(), "ข้อผิดพลาด".into());
    m.insert("warning".into(), "คำเตือน".into());
    m.insert("yes".into(), "ใช่".into());
    m.insert("no".into(), "ไม่".into());
    m.insert("ok".into(), "ตกลง".into());
    m.insert("settings".into(), "การตั้งค่า".into());
    m.insert("search".into(), "ค้นหา".into());
    m.insert("file".into(), "ไฟล์".into());
    m.insert("folder".into(), "โฟลเดอร์".into());
    m.insert("delete".into(), "ลบ".into());
    m.insert("language".into(), "ภาษา".into());
    m
}

fn vi_translations() -> HashMap<String, String> {
    let mut m = HashMap::new();
    m.insert("save".into(), "Lưu".into());
    m.insert("cancel".into(), "Hủy".into());
    m.insert("error".into(), "Lỗi".into());
    m.insert("warning".into(), "Cảnh báo".into());
    m.insert("yes".into(), "Có".into());
    m.insert("no".into(), "Không".into());
    m.insert("ok".into(), "OK".into());
    m.insert("settings".into(), "Cài đặt".into());
    m.insert("search".into(), "Tìm kiếm".into());
    m.insert("file".into(), "Tệp".into());
    m.insert("folder".into(), "Thư mục".into());
    m.insert("delete".into(), "Xóa".into());
    m.insert("language".into(), "Ngôn ngữ".into());
    m
}

fn pl_translations() -> HashMap<String, String> {
    let mut m = HashMap::new();
    m.insert("save".into(), "Zapisz".into());
    m.insert("cancel".into(), "Anuluj".into());
    m.insert("error".into(), "Błąd".into());
    m.insert("warning".into(), "Ostrzeżenie".into());
    m.insert("yes".into(), "Tak".into());
    m.insert("no".into(), "Nie".into());
    m.insert("ok".into(), "OK".into());
    m.insert("settings".into(), "Ustawienia".into());
    m.insert("search".into(), "Szukaj".into());
    m.insert("file".into(), "Plik".into());
    m.insert("folder".into(), "Folder".into());
    m.insert("delete".into(), "Usuń".into());
    m.insert("language".into(), "Język".into());
    m
}

fn nl_translations() -> HashMap<String, String> {
    let mut m = HashMap::new();
    m.insert("save".into(), "Opslaan".into());
    m.insert("cancel".into(), "Annuleren".into());
    m.insert("error".into(), "Fout".into());
    m.insert("warning".into(), "Waarschuwing".into());
    m.insert("yes".into(), "Ja".into());
    m.insert("no".into(), "Nee".into());
    m.insert("ok".into(), "OK".into());
    m.insert("settings".into(), "Instellingen".into());
    m.insert("search".into(), "Zoeken".into());
    m.insert("file".into(), "Bestand".into());
    m.insert("folder".into(), "Map".into());
    m.insert("delete".into(), "Verwijderen".into());
    m.insert("language".into(), "Taal".into());
    m
}

fn tr_translations() -> HashMap<String, String> {
    let mut m = HashMap::new();
    m.insert("save".into(), "Kaydet".into());
    m.insert("cancel".into(), "İptal".into());
    m.insert("error".into(), "Hata".into());
    m.insert("warning".into(), "Uyarı".into());
    m.insert("yes".into(), "Evet".into());
    m.insert("no".into(), "Hayır".into());
    m.insert("ok".into(), "Tamam".into());
    m.insert("settings".into(), "Ayarlar".into());
    m.insert("search".into(), "Ara".into());
    m.insert("file".into(), "Dosya".into());
    m.insert("folder".into(), "Klasör".into());
    m.insert("delete".into(), "Sil".into());
    m.insert("language".into(), "Dil".into());
    m
}

fn zh_tw_translations() -> HashMap<String, String> {
    let mut m = HashMap::new();
    m.insert("save".into(), "儲存".into());
    m.insert("cancel".into(), "取消".into());
    m.insert("error".into(), "錯誤".into());
    m.insert("warning".into(), "警告".into());
    m.insert("loading".into(), "載入中...".into());
    m.insert("yes".into(), "是".into());
    m.insert("no".into(), "否".into());
    m.insert("ok".into(), "確定".into());
    m.insert("retry".into(), "重試".into());
    m.insert("close".into(), "關閉".into());
    m.insert("settings".into(), "設定".into());
    m.insert("help".into(), "說明".into());
    m.insert("about".into(), "關於".into());
    m.insert("version".into(), "版本".into());
    m.insert("language".into(), "語言".into());
    m.insert("theme".into(), "佈景主題".into());
    m.insert("mode".into(), "模式".into());
    m.insert("tool".into(), "工具".into());
    m.insert("file".into(), "檔案".into());
    m.insert("folder".into(), "資料夾".into());
    m.insert("search".into(), "搜尋".into());
    m.insert("edit".into(), "編輯".into());
    m.insert("delete".into(), "刪除".into());
    m.insert("create".into(), "建立".into());
    m.insert("update".into(), "更新".into());
    m.insert("run".into(), "執行".into());
    m.insert("stop".into(), "停止".into());
    m.insert("start".into(), "開始".into());
    m.insert("pause".into(), "暫停".into());
    m.insert("resume".into(), "繼續".into());
    m.insert("reset".into(), "重設".into());
    m.insert("clear".into(), "清除".into());
    m.insert("refresh".into(), "重新整理".into());
    m.insert("language".into(), "語言".into());
    m
}
