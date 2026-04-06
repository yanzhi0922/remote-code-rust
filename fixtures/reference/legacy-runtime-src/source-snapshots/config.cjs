const crypto = require('node:crypto')

const RUNTIME_VERSION = '0.2.0'
const VALID_PERMISSION_MODES = new Set([
  'default',
  'acceptEdits',
  'bypassPermissions',
  'dontAsk',
  'plan',
])
const RESERVED_PROVIDER_REQUEST_HEADER_NAMES = new Set([
  'accept',
  'anthropic-beta',
  'anthropic-version',
  'authorization',
  'content-length',
  'content-type',
  'host',
  'user-agent',
  'x-api-key',
  'x-app',
  'x-anthropic-additional-protection',
  'x-claude-code-session-id',
  'x-claude-remote-container-id',
  'x-claude-remote-session-id',
  'x-client-app',
])

function readEnv(env, ...names) {
  for (const name of names) {
    const value = env[name]
    if (typeof value === 'string' && value.trim()) {
      return value.trim()
    }
  }
  return null
}

function normalizeProtocol(baseUrl, explicitProtocol) {
  if (explicitProtocol) {
    const normalized = explicitProtocol.trim().toLowerCase()
    if (normalized === 'openai' || normalized === 'anthropic') {
      return normalized
    }
  }

  if (!baseUrl) {
    return 'openai'
  }

  const normalizedBaseUrl = baseUrl.toLowerCase()
  if (
    normalizedBaseUrl.endsWith('/messages') ||
    normalizedBaseUrl.includes('/anthropic') ||
    normalizedBaseUrl.includes('compat=anthropic')
  ) {
    return 'anthropic'
  }

  return 'openai'
}

function normalizeBaseUrl(baseUrl, protocol) {
  if (!baseUrl) {
    return null
  }

  const trimmed = baseUrl.trim().replace(/\/+$/, '')
  if (protocol === 'anthropic') {
    if (trimmed.endsWith('/messages')) {
      return trimmed
    }
    if (/\/v\d+$/i.test(trimmed)) {
      return `${trimmed}/messages`
    }
    return `${trimmed}/v1/messages`
  }

  return trimmed.endsWith('/chat/completions')
    ? trimmed
    : `${trimmed}/chat/completions`
}

function parseLegacyRequestHeaders(rawHeaders) {
  const headers = {}
  if (!rawHeaders) {
    return headers
  }

  for (const headerString of rawHeaders.split(/\r?\n/)) {
    if (!headerString.trim()) {
      continue
    }
    const colonIdx = headerString.indexOf(':')
    if (colonIdx === -1) {
      continue
    }
    const name = headerString.slice(0, colonIdx).trim()
    const value = headerString.slice(colonIdx + 1).trim()
    if (name && value) {
      headers[name] = value
    }
  }

  return headers
}

function parseRequestHeadersJson(rawHeaders) {
  const trimmed = rawHeaders?.trim()
  if (!trimmed) {
    return {}
  }

  let parsed
  try {
    parsed = JSON.parse(trimmed)
  } catch (error) {
    const detail = error instanceof Error ? error.message : String(error)
    throw new Error(
      `REMOTE_CODE_REQUEST_HEADERS_JSON must be valid JSON: ${detail}`,
    )
  }

  if (!parsed || Array.isArray(parsed) || typeof parsed !== 'object') {
    throw new Error(
      'REMOTE_CODE_REQUEST_HEADERS_JSON must decode to a JSON object.',
    )
  }

  const headers = {}
  for (const [name, rawValue] of Object.entries(parsed)) {
    const normalizedName = String(name).trim()
    let normalizedValue = null
    if (typeof rawValue === 'string') {
      normalizedValue = rawValue.trim()
    } else if (
      typeof rawValue === 'number' ||
      typeof rawValue === 'boolean'
    ) {
      normalizedValue = String(rawValue)
    }
    if (normalizedName && normalizedValue) {
      headers[normalizedName] = normalizedValue
    }
  }

  return headers
}

function resolveRequestHeaderTemplates(headers, context) {
  const replacements = [
    ['${REMOTE_CODE_SESSION_ID}', context.sessionId ?? ''],
    ['${REMOTE_CODE_VERSION}', context.version ?? ''],
  ]

  return Object.fromEntries(
    Object.entries(headers).map(([name, value]) => {
      let resolved = value
      for (const [template, replacement] of replacements) {
        if (!resolved.includes(template)) {
          continue
        }
        resolved = resolved.split(template).join(replacement)
      }
      return [name, resolved]
    }),
  )
}

function buildRequestHeaderOverrides(env, context) {
  const resolvedHeaders = resolveRequestHeaderTemplates(
    {
      ...parseLegacyRequestHeaders(readEnv(env, 'ANTHROPIC_CUSTOM_HEADERS')),
      ...parseRequestHeadersJson(
        readEnv(env, 'REMOTE_CODE_REQUEST_HEADERS_JSON'),
      ),
    },
    context,
  )

  return Object.fromEntries(
    Object.entries(resolvedHeaders).filter(
      ([name]) =>
        !RESERVED_PROVIDER_REQUEST_HEADER_NAMES.has(name.toLowerCase()),
    ),
  )
}

function parseCliArgs(argv, env) {
  const config = {
    command: 'run',
    resumeSessionId: null,
    exportSessionId: null,
    exportPath: null,
    help: false,
    version: false,
    printMode: false,
    verbose: false,
    inputFormat: 'text',
    outputFormat: 'text',
    replayUserMessages: false,
    includePartialMessages: false,
    inlinePrompt: null,
    permissionMode: 'default',
    maxTurns: Math.max(
      1,
      Number.parseInt(env.REMOTE_CODE_MAX_TURNS ?? '12', 10) || 12,
    ),
    cwd: process.cwd(),
    sessionId: readEnv(env, 'REMOTE_CODE_SESSION_ID') ?? crypto.randomUUID(),
    provider: {
      name: readEnv(env, 'REMOTE_CODE_PROVIDER') ?? 'custom',
      baseUrl: null,
      apiKey: null,
      model: null,
      protocol: 'openai',
      timeoutMs: Math.max(
        1000,
        Number.parseInt(
          readEnv(env, 'API_TIMEOUT_MS', 'REMOTE_CODE_API_TIMEOUT_MS') ??
            '600000',
          10,
        ) || 600000,
      ),
      maxOutputTokens: Math.max(
        256,
        Number.parseInt(
          readEnv(env, 'REMOTE_CODE_MAX_OUTPUT_TOKENS') ?? '4096',
          10,
        ) || 4096,
      ),
      requestHeaderOverrides: {},
    },
  }

  let commandCaptured = false
  const positional = []
  for (let index = 0; index < argv.length; index += 1) {
    const arg = argv[index]

    if (!commandCaptured && !arg.startsWith('-')) {
      if (arg === 'doctor') {
        config.command = 'doctor'
        commandCaptured = true
        continue
      }
      if (arg === 'sessions') {
        config.command = 'sessions'
        commandCaptured = true
        continue
      }
      if (arg === 'resume') {
        config.command = 'resume'
        commandCaptured = true
        continue
      }
      if (arg === 'export') {
        config.command = 'export'
        commandCaptured = true
        continue
      }
    }

    switch (arg) {
      case '-h':
      case '--help':
        config.help = true
        break
      case '-v':
      case '-V':
      case '--version':
        config.version = true
        break
      case '-p':
      case '--print':
        config.printMode = true
        if (argv[index + 1] && !argv[index + 1].startsWith('-')) {
          positional.push(argv[index + 1])
          index += 1
        }
        break
      case '--verbose':
        config.verbose = true
        break
      case '--replay-user-messages':
        config.replayUserMessages = true
        break
      case '--include-partial-messages':
        config.includePartialMessages = true
        break
      case '--input-format':
        if (argv[index + 1]) {
          config.inputFormat = argv[index + 1]
          index += 1
        }
        break
      case '--output-format':
        if (argv[index + 1]) {
          config.outputFormat = argv[index + 1]
          index += 1
        }
        break
      case '--session-id':
        if (argv[index + 1]) {
          config.sessionId = argv[index + 1]
          index += 1
        }
        break
      case '--permission-mode':
        if (argv[index + 1]) {
          config.permissionMode = argv[index + 1]
          index += 1
        }
        break
      case '--model':
        if (argv[index + 1]) {
          config.provider.model = argv[index + 1]
          index += 1
        }
        break
      case '--output':
        if (argv[index + 1]) {
          config.exportPath = argv[index + 1]
          index += 1
        }
        break
      case '--max-turns':
        if (argv[index + 1]) {
          config.maxTurns = Math.max(
            1,
            Number.parseInt(argv[index + 1], 10) || config.maxTurns,
          )
          index += 1
        }
        break
      default:
        if (arg.startsWith('--input-format=')) {
          config.inputFormat = arg.slice('--input-format='.length)
        } else if (arg.startsWith('--output-format=')) {
          config.outputFormat = arg.slice('--output-format='.length)
        } else if (arg.startsWith('--session-id=')) {
          config.sessionId = arg.slice('--session-id='.length)
        } else if (arg.startsWith('--permission-mode=')) {
          config.permissionMode = arg.slice('--permission-mode='.length)
        } else if (arg.startsWith('--model=')) {
          config.provider.model = arg.slice('--model='.length)
        } else if (arg.startsWith('--output=')) {
          config.exportPath = arg.slice('--output='.length)
        } else if (arg.startsWith('--max-turns=')) {
          config.maxTurns = Math.max(
            1,
            Number.parseInt(arg.slice('--max-turns='.length), 10) ||
              config.maxTurns,
          )
        } else if (!arg.startsWith('-')) {
          positional.push(arg)
        }
        break
    }
  }

  if (config.command === 'resume') {
    config.resumeSessionId = positional.shift() ?? null
  }
  if (config.command === 'export') {
    config.exportSessionId = positional.shift() ?? null
    if (!config.exportPath && positional.length > 0) {
      config.exportPath = positional.shift()
    }
  }

  config.inlinePrompt = positional.join(' ').trim() || null
  if (!VALID_PERMISSION_MODES.has(config.permissionMode)) {
    config.permissionMode = 'default'
  }

  const providerBaseUrl = readEnv(
    env,
    'REMOTE_CODE_BASE_URL',
    'OPENAI_BASE_URL',
    'ANTHROPIC_BASE_URL',
    'REMOTE_CODE_API_BASE_URL',
  )
  const providerProtocol = normalizeProtocol(
    providerBaseUrl,
    readEnv(env, 'REMOTE_CODE_PROVIDER_PROTOCOL'),
  )

  config.provider.protocol = providerProtocol
  config.provider.baseUrl = normalizeBaseUrl(providerBaseUrl, providerProtocol)
  config.provider.apiKey = readEnv(
    env,
    'REMOTE_CODE_API_KEY',
    'OPENAI_API_KEY',
    'ANTHROPIC_API_KEY',
    'ANTHROPIC_AUTH_TOKEN',
  )
  config.provider.model =
    config.provider.model ??
    readEnv(env, 'REMOTE_CODE_MODEL', 'OPENAI_MODEL', 'ANTHROPIC_MODEL')
  config.provider.requestHeaderOverrides = buildRequestHeaderOverrides(env, {
    sessionId: config.sessionId,
    version: RUNTIME_VERSION,
  })

  if (
    !config.printMode &&
    (config.inputFormat === 'stream-json' ||
      config.outputFormat === 'stream-json')
  ) {
    config.printMode = true
  }

  return config
}

function validateProviderConfig(provider) {
  const issues = []
  if (!provider.baseUrl) {
    issues.push('REMOTE_CODE_BASE_URL is missing.')
  }
  if (!provider.apiKey) {
    issues.push('REMOTE_CODE_API_KEY is missing.')
  }
  if (!provider.model) {
    issues.push('REMOTE_CODE_MODEL is missing.')
  }
  return {
    ok: issues.length === 0,
    issues,
  }
}

function printHelp() {
  const text = [
    'Remote Code local CLI and headless runtime',
    '',
    'Usage:',
    '  remote-code',
    '  remote-code "explain this repository"',
    '  remote-code resume <session-id>',
    '  remote-code sessions',
    '  remote-code export <session-id> [--output <path>]',
    '  remote-code -p --input-format stream-json --output-format stream-json [options]',
    '  remote-code doctor',
    '',
    'Core options:',
    '  -p, --print                     Enable headless mode',
    '  --input-format <text|stream-json>',
    '  --output-format <text|stream-json>',
    '  --session-id <id>              Set the external session identifier',
    '  --permission-mode <mode>       default | acceptEdits | bypassPermissions | dontAsk | plan',
    '  --model <name>                 Override REMOTE_CODE_MODEL for this session',
    '  --output <path>                Output path for `export`',
    '  --max-turns <n>                Maximum tool/model turns per user message',
    '  --replay-user-messages         Echo inbound user messages back on stdout',
    '',
    'Environment:',
    '  REMOTE_CODE_PROVIDER           Provider preset label for docs and telemetry',
    '  REMOTE_CODE_BASE_URL           Base provider URL; OpenAI-compatible by default',
    '  REMOTE_CODE_API_KEY            Provider API key',
    '  REMOTE_CODE_MODEL              Provider model name',
    '  REMOTE_CODE_PROVIDER_PROTOCOL  Optional override: openai | anthropic',
    '  REMOTE_CODE_REQUEST_HEADERS_JSON Optional JSON object merged into outbound provider headers',
    '                                  Supports ${REMOTE_CODE_SESSION_ID} and ${REMOTE_CODE_VERSION}',
    '  ANTHROPIC_CUSTOM_HEADERS         Legacy newline-delimited header overrides (still accepted)',
    '',
  ]
  process.stdout.write(`${text.join('\n')}\n`)
}

function printVersion() {
  process.stdout.write(`${RUNTIME_VERSION} (Remote Code runtime)\n`)
}

module.exports = {
  RUNTIME_VERSION,
  normalizeBaseUrl,
  normalizeProtocol,
  parseCliArgs,
  printHelp,
  printVersion,
  validateProviderConfig,
}
