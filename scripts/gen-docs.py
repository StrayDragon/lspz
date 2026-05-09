"""
lspz 文档生成脚本 — SSOT Framework

从代码注释（/// / //!）生成 .gen.md 文档。
支持 --check 模式用于 CI 漂移检测。

SSOT 原则: 代码是唯一的真相源，文档都是生成物。

使用方法:
    python scripts/gen-docs.py           # 生成所有文档
    python scripts/gen-docs.py --check   # 检查文档是否过时
"""

import argparse
import hashlib
import re
import sys
from pathlib import Path


PROJECT_ROOT = Path(__file__).resolve().parent.parent
DOCS_API = PROJECT_ROOT / "docs" / "api"
DOCS_REF = PROJECT_ROOT / "docs" / "reference"
DOCS_SPECS = PROJECT_ROOT / "docs" / "specs"


def find_rust_files(root: Path) -> list[Path]:
    """Find all .rs files in the project workspace."""
    search_dirs = [
        root / "src",
        root / "lspz-core" / "src",
        root / "lspz" / "src",
        root / "examples",
    ]
    files = []
    for d in search_dirs:
        if d.exists():
            files.extend(sorted(d.rglob("*.rs")))
    return sorted(set(files))


def extract_module_doc(content: str) -> str | None:
    """Extract //! module-level documentation."""
    lines = content.split("\n")
    doc_lines = []
    for line in lines:
        stripped = line.strip()
        if stripped.startswith("//!"):
            doc_lines.append(stripped[3:].strip())
        elif not stripped.startswith("//!") and doc_lines:
            break
    return "\n".join(doc_lines) if doc_lines else None


def extract_item_docs(content: str) -> list[dict]:
    """Extract /// item-level documentation + the item definition."""
    items = []
    current_doc = []
    for line in content.split("\n"):
        stripped = line.strip()
        if stripped.startswith("///"):
            current_doc.append(stripped[3:].strip())
        elif current_doc and stripped and not stripped.startswith("//"):
            items.append({
                "doc": "\n".join(current_doc),
                "code": stripped,
            })
            current_doc = []
        else:
            current_doc = []
    return items


def generate_api_docs() -> dict[str, str]:
    """Generate API documentation from source comments.

    Returns: {output_path: content} mapping.
    """
    outputs = {}
    rust_files = find_rust_files(PROJECT_ROOT)
    if not rust_files:
        return outputs

    modules = []
    for filepath in rust_files:
        content = filepath.read_text(encoding="utf-8")
        module_doc = extract_module_doc(content)
        if module_doc:
            module_name = filepath.relative_to(PROJECT_ROOT).with_suffix("").as_posix()
            modules.append({"path": module_name, "name": filepath.stem, "doc": module_doc})

    # Module index
    if modules:
        gen_path = DOCS_API / "modules.gen.md"
        lines = [
            "# 模块索引",
            "",
            "> 自动从 `//!` 模块级注释生成。编辑源码注释后运行 `just gen-api-docs` 刷新。",
            "",
        ]
        for mod in modules:
            first_line = mod["doc"].split("\n")[0] if mod["doc"] else ""
            lines.append(f"- **`{mod['path']}`**: {first_line}")
        lines.append("")
        outputs[str(gen_path)] = "\n".join(lines)

    # Per-module API docs
    for filepath in rust_files:
        content = filepath.read_text(encoding="utf-8")
        items = extract_item_docs(content)
        if not items:
            continue
        module_name = filepath.relative_to(PROJECT_ROOT).with_suffix("").as_posix()
        gen_path = DOCS_API / f"{filepath.stem}.gen.md"
        lines = [
            f"# API: `{module_name}`",
            "",
            "> 自动从 `///` 注释生成。编辑源码注释后运行 `just gen-api-docs` 刷新。",
            "",
        ]
        for item in items:
            lines.append(item["doc"])
            lines.append("")
            if item["code"]:
                lines.append("```rust")
                lines.append(item["code"])
                lines.append("```")
                lines.append("")
        outputs[str(gen_path)] = "\n".join(lines)

    return outputs


def generate_config_docs() -> dict[str, str]:
    """Generate config reference from Config struct."""
    outputs = {}
    config_paths = [
        PROJECT_ROOT / "src" / "config.rs",
        PROJECT_ROOT / "lspz-core" / "src" / "config.rs",
        PROJECT_ROOT / "lspz" / "src" / "config.rs",
    ]
    config_file = next((p for p in config_paths if p.exists()), None)
    if not config_file:
        return outputs

    content = config_file.read_text(encoding="utf-8")
    struct_pattern = re.compile(
        r'(///[^\n]*\n)*pub struct (\w+)[^{]*\{([^}]+)\}',
        re.MULTILINE | re.DOTALL,
    )
    field_pattern = re.compile(
        r'(///[^\n]*\n)*\s*pub (\w+)\s*:\s*([^,}]+)',
        re.MULTILINE,
    )

    lines = [
        "# 配置参考",
        "",
        "> 自动从 `src/config.rs` 生成。编辑源码后运行 `just gen-config-docs` 刷新。",
        "",
    ]
    for m in struct_pattern.finditer(content):
        doc = (m.group(1) or "").replace("///", "").strip()
        struct_name = m.group(2)
        fields = m.group(3)
        lines.append(f"## `{struct_name}`")
        lines.append("")
        if doc:
            lines.extend(doc.split("\n"))
            lines.append("")
        for f in field_pattern.finditer(fields):
            f_doc = (f.group(1) or "").replace("///", "").strip()
            f_name = f.group(2)
            f_type = f.group(3).strip()
            doc_text = f" — {f_doc}" if f_doc else ""
            lines.append(f"- **`{f_name}`**: `{f_type}`{doc_text}")
        lines.append("")

    if lines[3:]:  # has content beyond header
        outputs[str(DOCS_REF / "config.gen.md")] = "\n".join(lines)
    return outputs


def generate_error_docs() -> dict[str, str]:
    """Generate error type reference from LspzError enum."""
    outputs = {}
    error_paths = [
        PROJECT_ROOT / "src" / "error.rs",
        PROJECT_ROOT / "lspz-core" / "src" / "error.rs",
    ]
    error_file = next((p for p in error_paths if p.exists()), None)
    if not error_file:
        return outputs

    content = error_file.read_text(encoding="utf-8")
    enum_pattern = re.compile(
        r'(///[^\n]*\n)*pub enum (\w+)\s*\{([^}]+)\}',
        re.MULTILINE | re.DOTALL,
    )
    variant_pattern = re.compile(
        r'(///[^\n]*\n)*\s*(\w+)(\([^)]*\))?',
        re.MULTILINE,
    )

    lines = [
        "# 错误类型参考",
        "",
        "> 自动从 `src/error.rs` 生成。编辑源码后运行 `just gen-error-docs` 刷新。",
        "",
    ]
    for m in enum_pattern.finditer(content):
        doc = (m.group(1) or "").replace("///", "").strip()
        enum_name = m.group(2)
        variants = m.group(3)
        lines.append(f"## `{enum_name}`")
        lines.append("")
        if doc:
            lines.extend(doc.split("\n"))
            lines.append("")
        for v in variant_pattern.finditer(variants):
            v_doc = (v.group(1) or "").replace("///", "").strip()
            v_name = v.group(2)
            v_params = v.group(3) or ""
            doc_text = f" — {v_doc}" if v_doc else ""
            lines.append(f"- **`{v_name}{v_params}`**{doc_text}")
        lines.append("")

    if lines[3:]:
        outputs[str(DOCS_REF / "error-types.gen.md")] = "\n".join(lines)
    return outputs


def generate_interceptor_docs() -> dict[str, str]:
    """Generate interceptor reference from Interceptor trait + implementations."""
    outputs = {}
    interceptor_paths = [
        PROJECT_ROOT / "src" / "interceptors" / "mod.rs",
        PROJECT_ROOT / "lspz-core" / "src" / "interceptors" / "mod.rs",
    ]
    mod_file = next((p for p in interceptor_paths if p.exists()), None)
    if not mod_file:
        return outputs

    src_dir = mod_file.parent
    interceptor_files = sorted(src_dir.glob("*.rs"))

    lines = [
        "# 拦截器参考",
        "",
        "> 自动从 `Interceptor` trait 定义及实现生成。编辑源码后运行 `just gen-api-docs` 刷新。",
        "",
    ]

    for f in interceptor_files:
        content = f.read_text(encoding="utf-8")
        items = extract_item_docs(content)
        if items:
            for item in items:
                lines.append(item["doc"])
                lines.append("")
                if item["code"]:
                    lines.append("```rust")
                    lines.append(item["code"])
                    lines.append("```")
                    lines.append("")
        else:
            # Still list the module name
            lines.append(f"- **`{f.stem}`**: 参见源码")

    lines.append("")
    outputs[str(DOCS_SPECS / "interceptors.gen.md")] = "\n".join(lines)
    return outputs


def content_hash(content: str) -> str:
    return hashlib.md5(content.encode()).hexdigest()


def write_outputs(outputs: dict[str, str], check: bool = False) -> bool:
    """Write generated files or check if they match.

    Returns True if all files are up-to-date.
    """
    all_match = True
    for path_str, content in sorted(outputs.items()):
        path = Path(path_str)
        path.parent.mkdir(parents=True, exist_ok=True)
        rel = path.relative_to(PROJECT_ROOT)

        if check:
            if path.exists():
                existing = path.read_text(encoding="utf-8")
                if existing == content:
                    print(f"  OK {rel}")
                else:
                    print(f"  DRIFT {rel} — content differs")
                    all_match = False
            else:
                print(f"  MISSING {rel} — will be generated on write")
                all_match = False
        else:
            path.write_text(content, encoding="utf-8")
            print(f"  WRITTEN {rel}")

    return all_match


def main():
    parser = argparse.ArgumentParser(description="lspz 文档生成 (SSOT Framework)")
    parser.add_argument("--check", action="store_true", help="检查模式：只验证不写入")
    args = parser.parse_args()

    mode = "CHECK" if args.check else "GENERATE"
    print(f"[gen-docs] Mode: {mode}")

    generators = [
        ("API docs", generate_api_docs),
        ("Config docs", generate_config_docs),
        ("Error docs", generate_error_docs),
        ("Interceptor docs", generate_interceptor_docs),
    ]

    all_outputs = {}
    for name, gen_fn in generators:
        try:
            print(f"\n[{name}]")
            result = gen_fn()
            all_outputs.update(result)
            if not result:
                print(f"  (no source code found yet)")
        except Exception as e:
            print(f"  ERROR: {e}")
            if not args.check:
                raise

    if not all_outputs:
        print("\nNo source code found. This is expected before MVP implementation.")
        return

    print(f"\nTotal: {len(all_outputs)} files to {'check' if args.check else 'generate'}")

    ok = write_outputs(all_outputs, check=args.check)

    if args.check and not ok:
        print("\nERROR: Documentation drift detected! Run `just gen-docs` to regenerate.")
        sys.exit(1)
    elif not args.check:
        print("\nDone. Run `just gen-check` for CI drift detection.")
    else:
        print("\nAll documentation is up to date.")


if __name__ == "__main__":
    main()
