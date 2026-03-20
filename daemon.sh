#!/bin/bash
set -euo pipefail

PLIST_NAME="com.github.copilot-wrapper.plist"
PLIST_SRC="$(cd "$(dirname "$0")" && pwd)/${PLIST_NAME}"
PLIST_DST="${HOME}/Library/LaunchAgents/${PLIST_NAME}"
LOG_DIR="${HOME}/Library/Logs/copilot-wrapper"

case "${1:-install}" in
  install)
    mkdir -p "${LOG_DIR}"
    # Unload if already loaded
    launchctl bootout "gui/$(id -u)/${PLIST_NAME%.plist}" 2>/dev/null || true
    cp "${PLIST_SRC}" "${PLIST_DST}"
    launchctl bootstrap "gui/$(id -u)" "${PLIST_DST}"
    echo "✓ Installed and started copilot-wrapper daemon"
    echo "  Logs: ${LOG_DIR}/"
    echo "  Stop:  ./daemon.sh stop"
    echo "  Start: ./daemon.sh start"
    ;;

  uninstall)
    launchctl bootout "gui/$(id -u)/${PLIST_NAME%.plist}" 2>/dev/null || true
    rm -f "${PLIST_DST}"
    echo "✓ Uninstalled copilot-wrapper daemon"
    ;;

  start)
    launchctl kickstart "gui/$(id -u)/${PLIST_NAME%.plist}"
    echo "✓ Started"
    ;;

  stop)
    launchctl kill SIGTERM "gui/$(id -u)/${PLIST_NAME%.plist}"
    echo "✓ Stopped"
    ;;

  status)
    launchctl print "gui/$(id -u)/${PLIST_NAME%.plist}" 2>&1 | head -20
    ;;

  logs)
    tail -f "${LOG_DIR}"/*.log
    ;;

  *)
    echo "Usage: $0 {install|uninstall|start|stop|status|logs}"
    exit 1
    ;;
esac
