/**
 * 内置地区关键词数据（按洲分组）
 * 作者: TanXiang
 *
 * 用于自定义区域规则创建时的快捷填充；UI 按洲 tab 分组展示，
 * builtinRegions 平铺导出保持兼容（useRegionRules 透传）
 */

export interface BuiltinRegion {
  name: string;
  keywords: string[];
}

export interface BuiltinRegionGroup {
  continent: string;
  regions: BuiltinRegion[];
}

/** 按洲分组的内置区域（tab 顺序即展示顺序） */
export const builtinRegionGroups: BuiltinRegionGroup[] = [
  // ---- 亚洲 ----
  {
    continent: "亚洲",
    regions: [
    { name: "香港", keywords: ["HK", "Hong Kong", "香港", "🇭🇰"] },
    { name: "台湾", keywords: ["TW", "Taiwan", "台湾", "台灣", "🇹🇼"] },
    { name: "日本", keywords: ["JP", "Japan", "日本", "🇯🇵"] },
    { name: "韩国", keywords: ["KR", "Korea", "韩国", "韓國", "🇰🇷"] },
    { name: "新加坡", keywords: ["SG", "Singapore", "新加坡", "🇸🇬"] },
    { name: "马来西亚", keywords: ["MY", "Malaysia", "马来西亚", "馬來西亞", "🇲🇾"] },
    { name: "泰国", keywords: ["TH", "Thailand", "泰国", "泰國", "🇹🇭"] },
    { name: "越南", keywords: ["VN", "Vietnam", "越南", "🇻🇳"] },
    { name: "印度尼西亚", keywords: ["ID", "Indonesia", "印尼", "印度尼西亚", "🇮🇩"] },
    { name: "菲律宾", keywords: ["PH", "Philippines", "菲律宾", "菲律賓", "🇵🇭"] },
    { name: "印度", keywords: ["IN", "India", "印度", "🇮🇳"] },
    { name: "澳门", keywords: ["MO", "Macao", "Macau", "澳门", "澳門", "🇲🇴"] },
    { name: "柬埔寨", keywords: ["KH", "Cambodia", "柬埔寨", "🇰🇭"] },
    { name: "孟加拉国", keywords: ["BD", "Bangladesh", "孟加拉", "🇧🇩"] },
    { name: "巴基斯坦", keywords: ["PK", "Pakistan", "巴基斯坦", "🇵🇰"] },
    { name: "哈萨克斯坦", keywords: ["KZ", "Kazakhstan", "哈萨克斯坦", "🇰🇿"] },
    ],
  },
  // ---- 欧洲 ----
  {
    continent: "欧洲",
    regions: [
    { name: "英国", keywords: ["UK", "GB", "United Kingdom", "英国", "🇬🇧"] },
    { name: "德国", keywords: ["DE", "Germany", "德国", "德國", "🇩🇪"] },
    { name: "法国", keywords: ["FR", "France", "法国", "法國", "🇫🇷"] },
    { name: "荷兰", keywords: ["NL", "Netherlands", "Holland", "荷兰", "荷蘭", "🇳🇱"] },
    { name: "意大利", keywords: ["IT", "Italy", "意大利", "🇮🇹"] },
    { name: "西班牙", keywords: ["ES", "Spain", "西班牙", "🇪🇸"] },
    { name: "瑞士", keywords: ["CH", "Switzerland", "瑞士", "🇨🇭"] },
    { name: "瑞典", keywords: ["SE", "Sweden", "瑞典", "🇸🇪"] },
    { name: "挪威", keywords: ["NO", "Norway", "挪威", "🇳🇴"] },
    { name: "芬兰", keywords: ["FI", "Finland", "芬兰", "🇫🇮"] },
    { name: "丹麦", keywords: ["DK", "Denmark", "丹麦", "🇩🇰"] },
    { name: "波兰", keywords: ["PL", "Poland", "波兰", "🇵🇱"] },
    { name: "乌克兰", keywords: ["UA", "Ukraine", "乌克兰", "🇺🇦"] },
    { name: "捷克", keywords: ["CZ", "Czech", "捷克", "🇨🇿"] },
    { name: "奥地利", keywords: ["AT", "Austria", "奥地利", "🇦🇹"] },
    { name: "比利时", keywords: ["BE", "Belgium", "比利时", "🇧🇪"] },
    { name: "爱尔兰", keywords: ["IE", "Ireland", "爱尔兰", "🇮🇪"] },
    { name: "葡萄牙", keywords: ["PT", "Portugal", "葡萄牙", "🇵🇹"] },
    { name: "希腊", keywords: ["GR", "Greece", "希腊", "🇬🇷"] },
    { name: "罗马尼亚", keywords: ["RO", "Romania", "罗马尼亚", "🇷🇴"] },
    { name: "保加利亚", keywords: ["BG", "Bulgaria", "保加利亚", "🇧🇬"] },
    { name: "塞尔维亚", keywords: ["RS", "Serbia", "塞尔维亚", "🇷🇸"] },
    { name: "俄罗斯", keywords: ["RU", "Russia", "俄罗斯", "🇷🇺"] },
    { name: "白俄罗斯", keywords: ["BY", "Belarus", "白俄罗斯", "🇧🇾"] },
    ],
  },
  // ---- 美洲 ----
  {
    continent: "美洲",
    regions: [
    { name: "美国", keywords: ["US", "USA", "United States", "America", "美国", "🇺🇸"] },
    { name: "加拿大", keywords: ["CA", "Canada", "加拿大", "🇨🇦"] },
    { name: "墨西哥", keywords: ["MX", "Mexico", "墨西哥", "🇲🇽"] },
    { name: "巴西", keywords: ["BR", "Brazil", "巴西", "🇧🇷"] },
    { name: "阿根廷", keywords: ["AR", "Argentina", "阿根廷", "🇦🇷"] },
    { name: "智利", keywords: ["CL", "Chile", "智利", "🇨🇱"] },
    { name: "哥伦比亚", keywords: ["CO", "Colombia", "哥伦比亚", "🇨🇴"] },
    { name: "秘鲁", keywords: ["PE", "Peru", "秘鲁", "🇵🇪"] },
    ],
  },
  // ---- 中东与非洲 ----
  {
    continent: "中东与非洲",
    regions: [
    { name: "土耳其", keywords: ["TR", "Turkey", "Türkiye", "土耳其", "🇹🇷"] },
    { name: "阿联酋", keywords: ["AE", "UAE", "United Arab Emirates", "阿联酋", "迪拜", "Dubai", "🇦🇪"] },
    { name: "以色列", keywords: ["IL", "Israel", "以色列", "🇮🇱"] },
    { name: "沙特阿拉伯", keywords: ["SA", "Saudi Arabia", "沙特", "🇸🇦"] },
    { name: "伊朗", keywords: ["IR", "Iran", "伊朗", "🇮🇷"] },
    { name: "南非", keywords: ["ZA", "South Africa", "南非", "🇿🇦"] },
    { name: "埃及", keywords: ["EG", "Egypt", "埃及", "🇪🇬"] },
    { name: "尼日利亚", keywords: ["NG", "Nigeria", "尼日利亚", "🇳🇬"] },
    { name: "肯尼亚", keywords: ["KE", "Kenya", "肯尼亚", "🇰🇪"] },
    ],
  },
  // ---- 大洋洲 ----
  {
    continent: "大洋洲",
    regions: [
    { name: "澳大利亚", keywords: ["AU", "Australia", "澳大利亚", "澳洲", "🇦🇺"] },
    { name: "新西兰", keywords: ["NZ", "New Zealand", "新西兰", "紐西蘭", "🇳🇿"] },
    ],
  },
  // ---- 大区 ----
  {
    continent: "大区",
    regions: [
    { name: "亚洲", keywords: ["Asia", "亚洲", "亞洲", "🇭🇰", "🇯🇵", "🇰🇷", "🇸🇬", "🇹🇼", "🇹🇭", "🇻🇳", "🇮🇩", "🇵🇭", "🇮🇳", "🇲🇾", "🇲🇴"] },
    { name: "欧洲", keywords: ["Europe", "欧洲", "歐洲", "🇬🇧", "🇩🇪", "🇫🇷", "🇳🇱", "🇮🇹", "🇪🇸", "🇨🇭", "🇸🇪", "🇳🇴", "🇫🇮", "🇩🇰", "🇵🇱", "🇺🇦", "🇦🇹", "🇧🇪", "🇮🇪", "🇵🇹", "🇬🇷", "🇷🇴", "🇧🇬", "🇷🇸"] },
    { name: "北美洲", keywords: ["North America", "北美洲", "🇺🇸", "🇨🇦", "🇲🇽"] },
    { name: "南美洲", keywords: ["South America", "南美洲", "🇧🇷", "🇦🇷", "🇨🇱", "🇨🇴", "🇵🇪"] },
    { name: "大洋洲", keywords: ["Oceania", "大洋洲", "🇦🇺", "🇳🇿"] },
    { name: "中东", keywords: ["Middle East", "中东", "中東", "🇹🇷", "🇦🇪", "🇮🇱", "🇸🇦", "🇮🇷"] },
    { name: "非洲", keywords: ["Africa", "非洲", "🇿🇦", "🇪🇬", "🇳🇬", "🇰🇪"] },
    ],
  },
];

/** 平铺列表（兼容既有消费方；顺序 = 分组顺序） */
export const builtinRegions: BuiltinRegion[] =
  builtinRegionGroups.flatMap((g) => g.regions);
