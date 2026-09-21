<script setup lang="ts">
import { computed } from "vue";
import {
  Sliders,
  Layers,
  GitFork,
  Globe,
  Cpu,
  Zap,
  Command,
  ShieldCheck,
  FlaskConical,
  Search,
  Palette,
  Network,
  PlusCircle,
  Download,
  List,
  Plus,
  Trash2,
  RefreshCw,
  Gauge,
  Activity,
  AlertTriangle,
  Clock,
  TrendingDown,
  LayoutGrid,
  Share2,
  Compass,
  Radio,
  Ban,
  Server,
  Globe2,
  MapPin,
  ShieldAlert,
  Scale,
  Folder,
  Wrench,
  Shield,
  FileText,
  X,
  Grid,
  Menu,
  Copy,
  Film,
  MessageSquare,
  Star,
  Crosshair,
  Check,
  Send,
  Rss,
  Calendar,
  HardDrive,
  FileUp,
  ClipboardList,
  Settings,
  Settings2,
  HelpCircle,
  Circle,
  Code,
  FileCode,
  ChevronUp,
  ChevronDown,
  ChevronRight,
  Minimize2,
  Maximize2,
  Eye,
  EyeOff,
  Terminal,
  SlidersHorizontal,
  Key,
  Lock,
  BookOpen,
  Database,
  Info,
  Play,
  Pause,
  Square,
  Sparkles,
  // process-helper 图标库扩展：此前 Bot/Box/Gamepad2/Music 未注册，
  // 全部静默落入 HelpCircle fallback，应用图标显示为问号
  Bot,
  Box,
  Gamepad2,
  Music,
} from "lucide-vue-next";


interface Props {
  name: string;
  size?: number | string;
  strokeWidth?: number;
  color?: string;
  class?: any;
}


const props = withDefaults(defineProps<Props>(), {
  size: 16,
  strokeWidth: 2,
  color: "currentColor",
});

const iconMap: Record<string, any> = {
  Sliders,
  Layers,
  GitFork,
  Globe,
  Cpu,
  Zap,
  Command,
  ShieldCheck,
  FlaskConical,
  Search,
  Palette,
  Network,
  PlusCircle,
  Download,
  List,
  Plus,
  Trash2,
  RefreshCw,
  Gauge,
  Activity,
  AlertTriangle,
  Clock,
  TrendingDown,
  LayoutGrid,
  Share2,
  Compass,
  Radio,
  Ban,
  Server,
  Globe2,
  MapPin,
  ShieldAlert,
  Scale,
  Folder,
  Wrench,
  Shield,
  FileText,
  X,
  Grid,
  Menu,
  Copy,
  Film,
  MessageSquare,
  Star,
  Crosshair,
  Check,
  Send,
  Rss,
  Calendar,
  HardDrive,
  FileUp,
  ClipboardList,
  Settings,
  Settings2,
  Code,
  FileCode,
  ChevronUp,
  ChevronDown,
  ChevronRight,
  Minimize2,
  Maximize2,
  Eye,
  EyeOff,
  Terminal,
  SlidersHorizontal,
  Key,
  Lock,
  BookOpen,
  Database,
  Info,
  Play,
  Pause,
  Square,
  Sparkles,
  Bot,
  Box,
  Gamepad2,
  Music,
};



const iconComponent = computed(() => {
  if (!props.name) return Circle;
  return iconMap[props.name] || HelpCircle || Circle;
});
</script>

<template>
  <component
    :is="iconComponent"
    :size="size"
    :stroke-width="strokeWidth"
    :color="color"
    :class="['base-icon', props.class]"
  />
</template>

<style scoped>
.base-icon {
  display: inline-block;
  vertical-align: middle;
  flex-shrink: 0;
  transition: transform 0.2s ease, color 0.2s ease;
}
</style>
