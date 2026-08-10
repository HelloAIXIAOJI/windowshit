mod format;

use std::process::ExitCode;

use windowshit_i18n::L10n;
use windowshit_netinfo::get_adapters;

fn main() -> ExitCode {
    // 必须先读代码页决定语言，再改 UTF-8 输出
    let mut i18n = L10n::detect();
    match i18n.lang() {
        "zh-CN" => i18n.add_ftl(include_str!("../locales/zh-CN.ftl")),
        _ => i18n.add_ftl(include_str!("../locales/en-US.ftl")),
    }
    i18n.set_help(
        include_str!("../locales/help.zh.txt"),
        include_str!("../locales/help.en.txt"),
    );

    L10n::setup_console_utf8();

    let args: Vec<String> = std::env::args().skip(1).collect();

    // 无参数：显示基本信息
    if args.is_empty() {
        return match get_adapters() {
            Ok(adapters) => {
                print!("{}", format::render_basic(&i18n, &adapters));
                ExitCode::SUCCESS
            }
            Err(e) => {
                eprintln!("{e}");
                ExitCode::from(1)
            }
        };
    }

    let cmd = args[0].to_lowercase();

    // 帮助
    if matches!(cmd.as_str(), "/?" | "-?" | "/help" | "-help") {
        println!("{}", i18n.help());
        return ExitCode::SUCCESS;
    }

    // 原版 ipconfig 只接受一个参数
    if args.len() > 1 {
        eprintln!("{}", i18n.tr("error-bad-parameter", None));
        return ExitCode::from(1);
    }

    match cmd.as_str() {
        "/all" | "-all" => match get_adapters() {
            Ok(adapters) => {
                print!("{}", format::render_all(&i18n, &adapters));
                ExitCode::SUCCESS
            }
            Err(e) => {
                eprintln!("{e}");
                ExitCode::from(1)
            }
        },
        _ => {
            // 无效参数：原版报错并打印帮助
            eprintln!("{}", i18n.tr("error-bad-parameter", None));
            println!();
            println!("{}", i18n.help());
            ExitCode::from(1)
        }
    }
}
