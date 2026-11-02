import ipaddress
import json
import os
import re
import subprocess
import sys


def command(*args):
    return subprocess.check_output(args, text=True, stderr=subprocess.DEVNULL, timeout=4)


def endpoint(value):
    host, sep, port = value.rpartition(':')
    if not sep or not port.isdigit():
        return None
    host = host.strip('[]').split('%', 1)[0]
    try:
        ipaddress.ip_address(host)
        return {'ip': host, 'port': int(port)}
    except ValueError:
        return None


def main():
    try:
        adapters = json.loads(command('ip', '-j', 'addr', 'show'))
        addresses = [item['local'] for adapter in adapters for item in adapter.get('addr_info', [])
                     if item.get('family') in ('inet', 'inet6') and item.get('scope') == 'global']
    except (OSError, ValueError, subprocess.SubprocessError):
        addresses = []
    sockets = []
    for network, args in (('tcp', '-ntp'), ('udp', '-nup')):
      for line in command('ss', '-H', args).splitlines():
        columns = line.split(None, 5)
        if len(columns) < 6:
            continue
        local, remote = endpoint(columns[3]), endpoint(columns[4])
        pids = set(re.findall(r'pid=(\d+)', columns[5]))
        if not local or not remote or len(pids) != 1:
            continue
        pid = int(next(iter(pids)))
        try:
            app = open(f'/proc/{pid}/comm', encoding='utf-8').read().strip()
            path = os.readlink(f'/proc/{pid}/exe')
        except (FileNotFoundError, PermissionError, ProcessLookupError, OSError):
            continue
        if app and path:
            sockets.append({'network': network, 'local': local, 'remote': remote, 'app': app, 'path': path})
        if len(sockets) >= 3000:
            break
    print(json.dumps({'addresses': addresses, 'sockets': sockets}, separators=(',', ':')))


try:
    main()
except (OSError, RuntimeError, ValueError, subprocess.SubprocessError) as error:
    print(str(error), file=sys.stderr)
    sys.exit(1)
