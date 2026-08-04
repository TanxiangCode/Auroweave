/**
 * 自定义区域规则 CRUD Hook
 * 作者: TanXiang
 *
 * 职责：管理自定义区域规则的新增、编辑、删除、快捷填充
 */
import { ref } from "vue";
import { useProxyStore } from "@/stores/proxy.store";
import { useToast } from "@/composables/useToast";
import type { CustomGroupRule } from "@/types";
import { builtinRegions } from "../utils/builtin-regions";

/**
 * 自定义区域规则管理 Hook
 */
export function useRegionRules() {
  const proxyStore = useProxyStore();
  const toast = useToast();

  /** 区域管理弹窗可见性 */
  const showRegionModal = ref(false);
  /** 当前编辑的规则 */
  const editingRule = ref<CustomGroupRule | null>(null);
  /** 是否为新增规则（vs 编辑已有规则） */
  const isNewRule = ref(false);

  /** 打开区域管理弹窗 */
  function openRegionModal() {
    showRegionModal.value = true;
  }

  /** 开始新增规则 */
  function addNewRule() {
    editingRule.value = {
      id: "",
      name: "",
      enabled: true,
      match_type: "keyword",
      keywords: [],
      pattern: "",
      protocols: [],
      order: proxyStore.customGroupRules.length,
    };
    isNewRule.value = true;
  }

  /** 开始编辑已有规则 */
  function editRule(rule: CustomGroupRule) {
    editingRule.value = { ...rule };
    isNewRule.value = false;
  }

  /** 应用内置区域快捷填充 */
  function applyBuiltinRegion(region: { name: string; keywords: string[] }) {
    if (!editingRule.value) return;
    editingRule.value.name = region.name;
    editingRule.value.match_type = "keyword";
    editingRule.value.keywords = [...region.keywords];
  }

  /** 保存规则（新增或更新） */
  function saveRule() {
    if (!editingRule.value) return;
    if (!editingRule.value.name.trim()) {
      toast.warning("请填写区域名称");
      return;
    }
    if (isNewRule.value) {
      const { id, ...ruleData } = editingRule.value;
      proxyStore.addCustomGroupRule(ruleData);
    } else {
      proxyStore.updateCustomGroupRule(editingRule.value);
    }
    editingRule.value = null;
    toast.success("区域规则已保存");
  }

  /** 删除规则 */
  function deleteRule(id: string) {
    proxyStore.deleteCustomGroupRule(id);
    toast.success("区域规则已删除");
  }

  /** 关闭区域管理弹窗 */
  function closeRegionModal() {
    showRegionModal.value = false;
  }

  return {
    // 状态
    showRegionModal,
    editingRule,
    isNewRule,
    builtinRegions,
    // 方法
    openRegionModal,
    addNewRule,
    editRule,
    applyBuiltinRegion,
    saveRule,
    deleteRule,
    closeRegionModal,
  };
}
