#!/bin/sh
# PostToolUse 钩子示例：修改工具输出
# 退出码说明:
#   0 - 允许，可附带修改后的 JSON
#   2 - 拒绝，标记为错误
#   其他 - 警告，继续执行

# 从环境变量读取工具信息
TOOL_NAME="$HOOK_TOOL_NAME"
TOOL_OUTPUT="$HOOK_TOOL_OUTPUT"
TOOL_IS_ERROR="$HOOK_TOOL_IS_ERROR"

# 示例：过滤敏感信息
if echo "$TOOL_OUTPUT" | grep -q "password\|secret\|token"; then
    # 检测到敏感信息，进行脱敏
    SANITIZED_OUTPUT=$(echo "$TOOL_OUTPUT" | sed -E 's/(password|secret|token)[=:][^[:space:]]*/\1=[REDACTED]/gi')
    echo "⚠️ 已脱敏敏感信息"
    echo "MODIFIED_JSON:$SANITIZED_OUTPUT"
    exit 0
fi

# 示例：添加审计日志
echo "✅ 工具执行完成，输出已审计"
exit 0
