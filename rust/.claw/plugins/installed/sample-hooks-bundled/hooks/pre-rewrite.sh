#!/bin/sh
# PreToolUse 钩子示例：修改工具输入
# 退出码说明:
#   0 - 允许，可附带修改后的 JSON
#   2 - 拒绝工具执行
#   其他 - 警告，继续执行

# 从环境变量读取工具信息
TOOL_NAME="$HOOK_TOOL_NAME"
TOOL_INPUT="$HOOK_TOOL_INPUT"

# 示例：如果是写文件操作，检查是否包含敏感路径
if [ "$TOOL_NAME" = "write_file" ]; then
    # 检查是否尝试写入敏感文件
    if echo "$TOOL_INPUT" | grep -q "/etc/passwd\|/etc/shadow"; then
        echo "⚠️ 检测到敏感文件写入尝试，已阻止"
        exit 2
    fi
fi

# 示例：修改工具输入（添加审计标记）
# 输出格式：第一行是消息，MODIFIED_JSON: 开头的是修改后的 JSON
cat <<EOF
✅ 工具调用已审计
MODIFIED_JSON:$TOOL_INPUT
EOF

exit 0
