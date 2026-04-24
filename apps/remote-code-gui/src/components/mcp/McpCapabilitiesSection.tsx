import { Check, FileText, MessageSquare, MousePointerClick, Wrench, X } from 'lucide-react';

interface McpCapabilitiesSectionProps {
  capabilities: {
    tools: boolean;
    resources: boolean;
    prompts: boolean;
    sampling: boolean;
  };
}

interface CapabilityItem {
  key: string;
  label: string;
  icon: React.ReactNode;
  supported: boolean;
}

export function McpCapabilitiesSection({ capabilities }: McpCapabilitiesSectionProps) {
  const items: CapabilityItem[] = [
    { key: 'tools', label: '工具', icon: <Wrench size={14} />, supported: capabilities.tools },
    { key: 'resources', label: '资源', icon: <FileText size={14} />, supported: capabilities.resources },
    { key: 'prompts', label: '提示', icon: <MessageSquare size={14} />, supported: capabilities.prompts },
    { key: 'sampling', label: '采样', icon: <MousePointerClick size={14} />, supported: capabilities.sampling },
  ];

  return (
    <div className="flex flex-wrap gap-2" data-testid="mcp-capabilities">
      {items.map((item) => (
        <div
          key={item.key}
          className={`flex items-center gap-1.5 rounded-xl px-3 py-1.5 text-sm ${
            item.supported
              ? 'bg-emerald-50 text-emerald-700'
              : 'bg-slate-50 text-slate-400'
          }`}
          data-testid={`mcp-capability-${item.key}`}
        >
          {item.icon}
          <span>{item.label}</span>
          {item.supported ? (
            <Check size={12} className="text-emerald-600" />
          ) : (
            <X size={12} className="text-slate-300" />
          )}
        </div>
      ))}
    </div>
  );
}
