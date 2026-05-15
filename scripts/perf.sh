#!/usr/bin/env bash
set -euo pipefail

# 默认值（用户可通过环境变量或 make 传递）
PERF_ROWS="${PERF_ROWS:-100000}"
DB_URL="${DB_URL:-}"
BIN="${BIN:-./target/release/dbmock}"

# 检测操作系统并选择 time 命令
TIME_CMD="time"
TIME_ARGS=""
UNAME_S=$(uname -s)
case "$UNAME_S" in
    Linux)
        if command -v /usr/bin/time >/dev/null 2>&1; then
            TIME_CMD="/usr/bin/time"
            TIME_ARGS="-v"
        fi
        ;;
    Darwin)
        if command -v gtime >/dev/null 2>&1; then
            TIME_CMD="gtime"
            TIME_ARGS="-v"
        else
            echo "⚠️  提示: 安装 gnu-time 可获得更详细的资源报告 (brew install gnu-time)" >&2
        fi
        ;;
esac

# 参数检查
if [ -z "$DB_URL" ]; then
    echo "❌ 请设置 DB_URL 环境变量，例如: export DB_URL='postgresql://...'" >&2
    exit 1
fi

if [ ! -x "$BIN" ]; then
    echo "❌ 可执行文件不存在或不可执行: $BIN" >&2
    exit 1
fi

echo "🚀 性能测试：每表生成 $PERF_ROWS 行数据"
echo "开始时间: $(date '+%Y-%m-%d %H:%M:%S')"
echo "数据库 URL: $DB_URL"
echo "----------------------------------------"

# 临时文件保存完整输出
TMP_OUT=$(mktemp)

# 执行测试并捕获输出
if [ -n "$TIME_ARGS" ]; then
    $TIME_CMD $TIME_ARGS "$BIN" generate --db-url "$DB_URL" --rows users="$PERF_ROWS" 2>&1 | tee "$TMP_OUT"
    EXIT_CODE=${PIPESTATUS[0]}
else
    (time "$BIN" generate --db-url "$DB_URL" --rows users="$PERF_ROWS") 2>&1 | tee "$TMP_OUT"
    EXIT_CODE=${PIPESTATUS[0]}
fi

echo "----------------------------------------"

# 解析 dbmock 输出的关键统计
# 注意：使用更健壮的解析方式（匹配冒号后的数字）
INSERTED=$(grep -E "^\s*Rows inserted\s*:" "$TMP_OUT" | awk -F':' '{print $2}' | xargs)
ERRORS=$(grep -E "^\s*Errors\s*:" "$TMP_OUT" | awk -F':' '{print $2}' | xargs)
TABLES_PROCESSED=$(grep -E "^\s*Tables processed\s*:" "$TMP_OUT" | awk -F':' '{print $2}' | xargs)

# 计算理论总行数（如果解析到表数且 PERF_ROWS 有效）
if [ -n "$TABLES_PROCESSED" ] && [ "$TABLES_PROCESSED" -gt 0 ] 2>/dev/null; then
    TOTAL_TARGET=$((PERF_ROWS * TABLES_PROCESSED))
    TARGET_DESC="${PERF_ROWS} 行/表 × ${TABLES_PROCESSED} 表 = ${TOTAL_TARGET} 行"
else
    TARGET_DESC="${PERF_ROWS} 行"
fi

# 解析 time 输出中的各项指标
ELAPSED_RAW=$(grep -E "Elapsed \(wall clock\) time" "$TMP_OUT" | awk -F': ' '{print $2}' | head -1)
if [ -n "$ELAPSED_RAW" ]; then
    if [[ "$ELAPSED_RAW" =~ ^([0-9]+):([0-9]+(\.[0-9]+)?)$ ]]; then
        MIN=${BASH_REMATCH[1]}
        SEC=${BASH_REMATCH[2]}
        ELAPSED_SEC=$(echo "$MIN * 60 + $SEC" | bc)
    else
        ELAPSED_SEC=$(echo "$ELAPSED_RAW" | bc)
    fi
else
    ELAPSED_SEC=""
fi

USER_TIME=$(grep -E "User time \(seconds\)" "$TMP_OUT" | awk -F': ' '{print $2}' | cut -d' ' -f1)
SYS_TIME=$(grep -E "System time \(seconds\)" "$TMP_OUT" | awk -F': ' '{print $2}' | cut -d' ' -f1)
CPU_PERC=$(grep -E "Percent of CPU" "$TMP_OUT" | awk -F': ' '{print $2}')
MAX_RSS=$(grep -E "Maximum resident set size" "$TMP_OUT" | awk -F': ' '{print $2}')
SWAPS=$(grep -E "Swaps" "$TMP_OUT" | awk -F': ' '{print $2}')
SOCK_SENT=$(grep -E "Socket messages sent" "$TMP_OUT" | awk -F': ' '{print $2}')
SOCK_RECV=$(grep -E "Socket messages received" "$TMP_OUT" | awk -F': ' '{print $2}')
MAJ_PF=$(grep -E "Major .* page faults" "$TMP_OUT" | awk -F': ' '{print $2}')
MIN_PF=$(grep -E "Minor .* page faults" "$TMP_OUT" | awk -F': ' '{print $2}')
VOL_CTX=$(grep -E "Voluntary context switches" "$TMP_OUT" | awk -F': ' '{print $2}')
INVOL_CTX=$(grep -E "Involuntary context switches" "$TMP_OUT" | awk -F': ' '{print $2}')

# 计算吞吐率（基于实际插入行数）
if [ -n "$INSERTED" ] && [ "$INSERTED" -gt 0 ] 2>/dev/null && [ -n "$ELAPSED_SEC" ] && (( $(echo "$ELAPSED_SEC > 0" | bc -l) )); then
    RATE=$(echo "scale=0; $INSERTED / $ELAPSED_SEC" | bc)
else
    RATE="N/A"
fi

# 以表格形式输出
printf "\n📊 性能报告（表格）\n"
printf "+--------------------------------+----------------------------------+\n"
printf "| 指标                           | 值                               |\n"
printf "+--------------------------------+----------------------------------+\n"
printf "| ✅ 总耗时                      | %-32s |\n" "${ELAPSED_SEC:-?} 秒"
printf "| 📊 目标行数                    | %-32s |\n" "$TARGET_DESC"
printf "| 📊 实际插入行数                | %-32s |\n" "${INSERTED:-0}"
printf "| 📊 错误数                      | %-32s |\n" "${ERRORS:-0}"
printf "| 📊 平均吞吐率                  | %-32s |\n" "${RATE} 行/秒"
printf "+--------------------------------+----------------------------------+\n"
printf "| 资源占用                       |                                  |\n"
printf "+--------------------------------+----------------------------------+\n"
[ -n "$USER_TIME" ] && printf "| User time (用户态)              | %-32s |\n" "${USER_TIME} 秒"
[ -n "$SYS_TIME" ] && printf "| System time (内核态)            | %-32s |\n" "${SYS_TIME} 秒"
[ -n "$CPU_PERC" ] && printf "| CPU 占用率                      | %-32s |\n" "$CPU_PERC"
[ -n "$MAX_RSS" ] && printf "| Maximum resident set size       | %-32s |\n" "$MAX_RSS"
[ -n "$SWAPS" ] && printf "| Swaps                           | %-32s |\n" "$SWAPS"
[ -n "$SOCK_SENT" ] && printf "| Socket messages sent            | %-32s |\n" "$SOCK_SENT"
[ -n "$SOCK_RECV" ] && printf "| Socket messages received        | %-32s |\n" "$SOCK_RECV"
[ -n "$MAJ_PF" ] && printf "| Major page faults               | %-32s |\n" "$MAJ_PF"
[ -n "$MIN_PF" ] && printf "| Minor page faults               | %-32s |\n" "$MIN_PF"
[ -n "$VOL_CTX" ] && printf "| Voluntary context switches      | %-32s |\n" "$VOL_CTX"
[ -n "$INVOL_CTX" ] && printf "| Involuntary context switches    | %-32s |\n" "$INVOL_CTX"
printf "+--------------------------------+----------------------------------+\n"

# 如果错误数 >0，给出提示
if [ -n "$ERRORS" ] && [ "$ERRORS" -gt 0 ] 2>/dev/null; then
    echo "⚠️  存在 $ERRORS 个错误，可能是唯一约束冲突或外键引用失败。"
    echo "   建议调整生成策略（如使用 sequence 生成器或分步生成）以减少错误。"
fi

# 清理临时文件
rm -f "$TMP_OUT"
exit $EXIT_CODE
