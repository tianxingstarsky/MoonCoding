#include "softkeyboard.h"
#include "googlepinyinengine.h"
#include "touchscroll.h"

#include <QCoreApplication>
#include <QFrame>
#include <QHBoxLayout>
#include <QLabel>
#include <QList>
#include <QPushButton>
#include <QScreen>
#include <QGuiApplication>
#include <QScrollArea>
#include <QScrollBar>
#include <QToolButton>
#include <QVBoxLayout>
#include <QtGlobal>

namespace {

// Compact open-style syllable table (common chars/phrases).
const char *kDictBlob = R"DICT(
a 啊 阿
ai 爱 矮 哎
an 安 按
ao 奥 傲
ba 把 吧 八 爸
bai 白 百 摆
ban 办 半 板 班
bang 帮 棒
bao 包 报 保 宝
bei 被 北 杯 背
ben 本
bi 比 笔 必 闭
bian 边 变 便
biao 表 标
bie 别
bing 并 病 冰
bo 波 伯
bu 不 步 部
cai 才 菜 彩
can 参
cao 草
ce 测
ceng 层
cha 查 茶 差
chai 拆
chan 产
chang 长 常 场 唱
chao 超 朝
che 车
chen 陈
cheng 成 城 程 称
chi 吃 持 尺
chong 重 冲
chou 抽
chu 出 处 初
chuan 传 船 穿
chuang 创 窗 床
chui 吹
chun 春
ci 次 此 词
cong 从
cu 粗
cui 脆
cun 村 存
cuo 错
da 大 打 达
dai 带 代 待
dan 但 单 担
dang 当
dao 到 道 倒 导
de 的 得 地 德
deng 等 灯
di 地 第 低 弟
dian 点 电 店 电脑
diao 掉 调
ding 定 顶
diu 丢
dong 东 动 懂 冬
dou 都 斗
du 读 度 独
duan 段 短
dui 对 队
dun 顿
duo 多
e 饿 额
en 恩
er 而 二 儿
fa 发 法
fan 反 饭 翻
fang 方 放 房
fei 非 飞 费
fen 分 份
feng 风
fou 否
fu 服 父 府 付
gai 该 改 盖
gan 干 感 敢
gang 刚
gao 高 告 搞
ge 个 各 哥 歌
gei 给
gen 跟 根
geng 更
gong 工 公 共
gou 够 狗
gu 古 故 顾
gua 挂
guai 怪
guan 关 管 观
guang 光 广
gui 贵 归
gun 滚
guo 国 过 果
ha 哈
hai 还 海 害 孩
han 汉 含 喊
hang 行
hao 好 号
he 和 河 何 合
hei 黑
hen 很
heng 横
hong 红
hou 后 候
hu 护 湖 户
hua 话 花 化 华 画
huai 坏
huan 换 还 环 欢
huang 黄
hui 会 回
hun 混
huo 或 活 火
ji 机 及 几 己 计 记 基 级 集 极 即 技 际 济 积 击 奇 迹
jia 家 加 假 价 甲 架 驾 夹
jian 见 间 件 建 简 检 健 减 坚 尖 键
jiang 将 讲 江 奖 降
jiao 教 叫 交 较 脚 角 焦
jie 接 结 解 姐 界 阶 街 节 借 介
jin 进 金 近 紧 尽 仅 禁
jing 经 静 精 京 景 警 竟 境
jiu 就 九 久 酒 旧 救
ju 据 举 句 具 局 居 拒 距
jue 觉 决 绝
jun 军 均
ka 卡
kai 开
kan 看
kang 抗
kao 考 靠
ke 可 课 克 科
ken 肯
kong 空 控
kou 口
ku 苦 哭
kua 夸
kuai 快 块
kuan 宽
kuang 况
kun 困
kuo 扩
la 啦 拉
lai 来
lan 蓝 懒
lang 浪
lao 老
le 了 乐
lei 累 类
leng 冷
li 里 理 力 立 利 历 例 离 李 丽 礼 粒 厉 梨
lian 连 脸 练
liang 两 亮 量
liao 了 料
lie 列
lin 林
ling 另 零 领
liu 六 留 流
long 龙
lou 楼
lu 路 录
lv 绿 旅
luan 乱
lun 论
luo 落 罗
ma 吗 妈 马
mai 买 卖
man 满 慢
mang 忙
mao 毛 猫
me 么
mei 没 每 美
men 们 门
meng 梦
mi 米 密
mian 面
miao 秒
min 民
ming 明 名 命
mo 莫
mou 某
mu 目 木 母
mima 密码
na 那 拿 哪
nai 奶
nan 难 男 南
nao 脑
ne 呢
nei 内
neng 能
ni 你 尼
nian 年 念
niang 娘
niao 鸟
nin 您
ning 宁
niu 牛
nong 农
nu 努
nv 女
nuan 暖
o 哦
ou 欧
pa 怕
pai 排 拍
pan 盘
pang 旁
pao 跑
pei 配
pen 喷
peng 朋
pi 皮
pian 片
piao 票
pin 品
ping 平
po 破
pu 普
qi 起 其 七 气
qian 前 钱 千
qiang 强
qiao 桥 巧
qie 且
qin 亲
qing 请 清 情 轻
qiu 求 球
qu 去 取 区
quan 全
que 却 确
qun 群
ran 然
rang 让
rao 绕
re 热
ren 人 认
reng 仍
ri 日
rong 容
rou 肉
ru 如 入
ruan 软
rui 瑞
run 润
ruo 若
sa 撒
sai 赛
san 三
sang 桑
sao 扫
se 色
sha 啥 沙
shan 山
shang 上 商
shao 少
she 设 社
shei 谁
shen 什 身 深
sheng 生 声 省 胜 升 剩 圣 盛
shi 是 时 世 市 事 实 十 式 士 师 诗 史 失 石 视 试 室 适 释 食 识 始 施 湿 拾 驶 势 示 氏 使 似
shijie 世界
shishi 实施 事实 试试
shihou 时候
shou 手 受 收 首 守 寿 售 瘦 兽 授 手机
shu 书 数 树 属 术 述 熟 输 叔 舒 鼠
shua 刷
shuai 帅 摔 衰
shuang 双 爽
shui 水 谁 睡 税
shun 顺 瞬
shuo 说
si 四 死 思 司 丝 私 似 寺
song 送 松 宋
sou 搜 艘
su 苏 速 素 诉 俗
suan 算 酸 蒜
sui 虽 随 岁 碎
sun 孙 损
suo 所 锁 缩 索
ta 他 她 它 塔 踏
tai 太 台 抬 态 泰
tan 谈 弹 探 坦 贪 叹
tang 汤 糖 躺 烫 堂 唐
tao 套 逃 桃 淘 讨
te 特
teng 疼 腾
ti 体 提 题 替 踢 梯
tian 天 田 填 甜 添
tiao 条 跳 调 挑
tie 铁 贴
ting 听 停 庭 挺
tong 同 通 痛 统 童
tou 头 投 透 偷
tu 图 土 突 徒 途 吐
tuan 团
tui 退 推 腿
tun 吞
tuo 托 拖 脱 妥
wa 挖 娃 哇 袜
wai 外 歪
wan 完 万 玩 晚 碗 弯
wang 往 望 网 忘 王 亡 旺
wei 为 位 未 微 围 伟 威 危 味 维 卫
wen 问 文 闻 稳 温
wo 我 窝 握 卧
wu 五 无 物 务 武 误 屋 雾 舞
xi 西 喜 系 息 希 习 细 洗 戏 吸
xia 下 夏 吓 虾 峡
xian 先 现 线 显 县 限 闲 鲜 险
xiang 想 向 像 相 香 箱 详 响 乡
xiao 小 笑 消 校 效 晓 肖
xie 写 些 谢 协 斜 鞋 血
xin 新 心 信 辛 欣 薪
xing 行 性 姓 星 兴 形 型 幸
xiong 兄 雄 胸 熊
xiu 修 休 秀 袖
xu 需 许 续 序 虚 须 徐
xuan 选 宣 旋 悬
xue 学 雪 血 穴
xun 寻 询 训 迅 讯
ya 呀 牙 亚 压 雅
yan 眼 言 研 严 颜 演 验 烟 延
yang 样 阳 养 羊 扬 洋
yao 要 药 摇 腰 咬 遥
ye 也 业 夜 叶 页 野
yi 一 以 已 意 易 义 亿 衣 医 依 移 异 艺 忆 议 宜 椅 亦 疑
yin 因 音 银 引 印 饮 隐
ying 应 影 英 迎 硬 营 映
yo 哟
yong 用 永 勇 涌 拥
you 有 又 由 友 右 游 优 油
yu 与 于 鱼 雨 语 遇 预 余 育 玉
yuan 原 远 元 院 员 园 愿 圆 源
yue 月 越 约 乐 阅
yun 云 运 允 晕
za 杂 砸
zai 在 再 载 灾
zan 咱 赞 暂
zang 脏
zao 早 造 糟 枣
ze 则 责 泽 择
zei 贼
zen 怎
zeng 增 赠
zha 炸 扎 眨
zhai 摘 窄 债
zhan 站 战 展 占 沾
zhang 长 张 章 涨 掌 丈
zhao 找 照 着 招 朝 赵
zhe 这 着 者 折 哲
zhen 真 针 阵 振 镇 珍
zheng 正 整 争 政 证 征
zhi 只 知 之 直 制 至 治 支 纸 指 值 质
zhong 中 种 重 众 终 钟
zhou 周 州 洲 轴
zhu 主 住 注 助 珠 猪 竹 祝
zhua 抓
zhuan 转 专 砖 赚
zhuang 装 状 壮 撞
zhui 追
zhun 准
zhuo 桌 着 捉
zi 字 自 子 资 紫
zong 总 宗 综
zou 走 奏
zu 组 足 族 祖
zuan 钻
zui 最 嘴 罪
zun 尊
zuo 做 作 坐 左 昨 座
nihao 你好
nimen 你们
women 我们
tamen 他们 她们
xiexie 谢谢
duibuqi 对不起
meiguanxi 没关系
zaoshanghao 早上好
wanshanghao 晚上好
wanshang 晚上
zaoshang 早上
mingtian 明天
jintian 今天
zuotian 昨天
xianzai 现在
shijian 时间
diannao 电脑
shouji 手机
wangluo 网络
mima 密码
wenjian 文件
xiangmu 项目
daima 代码
shezhi 设置
baocun 保存
dakai 打开
guanbi 关闭
qingkong 清空
sousuo 搜索
lianjie 连接
duankai 断开
shurufa 输入法
zhongwen 中文
yingwen 英文
keyi 可以
buxing 不行
haode 好的
bushi 不是
shenme 什么
zenme 怎么
weishenme 为什么
nali 哪里
zhege 这个
nage 那个
yige 一个
henduo 很多
yidian 一点
xuyao 需要
xiwang 希望
bangzhu 帮助
wenti 问题
jiejue 解决
kaishi 开始
jieshu 结束
xiayibu 下一步
qingqiu 请求
fanhui 返回
queren 确认
quxiao 取消
chenggong 成功
shibai 失败
cuowu 错误
zhengque 正确
qingchu 清楚
xiugai 修改
shanchu 删除
tianjia 添加
gengxin 更新
anzhuang 安装
qidong 启动
zhongzhi 终止
jixu 继续
dengdai 等待
wancheng 完成
ceshi 测试
bianyi 编译
bushu 部署
fuwuqi 服务器
moxing 模型
duihua 对话
xiaoxi 消息
neirong 内容
biaoti 标题
lujing 路径
mulu 目录
wenben 文本
zhuangtai 状态
woaini 我爱你
zhongguo 中国
beijing 北京
shanghai 上海
bangwo 帮我
qingbangwo 请帮我
meicuo 没错
)DICT";

constexpr int kCandPerPage = 5;
constexpr int kMaxComposing = 64;
constexpr int kCandBtnMinWidth = 64;

} // namespace

SoftKeyboard::SoftKeyboard(QWidget *parent)
    : QWidget(parent)
{
    setObjectName(QStringLiteral("softKeyboard"));
    setSizePolicy(QSizePolicy::Expanding, QSizePolicy::Fixed);
    loadDictionary(); // tiny fallback only
    m_useEngine = GooglePinyinEngine::instance().open(QString());
    if (m_useEngine) {
        qInfo("MoonCoding IME: Google Pinyin engine ready (full dict)");
    } else {
        qWarning("MoonCoding IME: Google Pinyin dict missing — using tiny fallback lexicon");
    }
    buildUi();
    hide();
}

void SoftKeyboard::loadDictionary()
{
    const QString blob = QString::fromUtf8(kDictBlob);
    for (QString line : blob.split(QLatin1Char('\n'), Qt::SkipEmptyParts)) {
        line = line.trimmed();
        if (line.isEmpty()) {
            continue;
        }
        const QStringList parts = line.split(QLatin1Char(' '), Qt::SkipEmptyParts);
        if (parts.size() < 2) {
            continue;
        }
        const QString key = parts.first().toLower();
        QStringList &slot = m_dict[key];
        for (const QString &w : parts.mid(1)) {
            if (!slot.contains(w)) {
                slot.append(w);
            }
        }
        m_maxKeyLen = qMax(m_maxKeyLen, key.size());
    }
}

void SoftKeyboard::clearComposing()
{
    m_composing.clear();
    m_composeLabel->clear();
    m_allCandidates.clear();
    m_allMatchedPy.clear();
    m_allCandIds.clear();
    m_candidatePage = 0;
    if (m_useEngine) {
        GooglePinyinEngine::instance().resetSearch();
    }
    updateCandidates();
}

QPushButton *SoftKeyboard::makeKey(const QString &label, const QString &code,
                                   int stretch, const QString &objectName)
{
    auto *btn = new QPushButton(label, this);
    btn->setObjectName(objectName);
    btn->setSizePolicy(QSizePolicy::Expanding, QSizePolicy::Expanding);
    btn->setProperty("code", code);
    btn->setProperty("stretchHint", stretch);
    btn->setFocusPolicy(Qt::NoFocus);
    return btn;
}

void SoftKeyboard::addCharRow(QVBoxLayout *keysLayout, const QString &chars, int sidePadStretch)
{
    auto *row = new QHBoxLayout;
    row->setSpacing(5);
    row->setContentsMargins(0, 0, 0, 0);
    if (sidePadStretch > 0) {
        row->addStretch(sidePadStretch);
    }
    for (QChar ch : chars) {
        auto *btn = makeKey(QString(ch), QString(ch), 1);
        connect(btn, &QPushButton::clicked, this, &SoftKeyboard::onCharClicked);
        row->addWidget(btn, 1);
    }
    if (sidePadStretch > 0) {
        row->addStretch(sidePadStretch);
    }
    keysLayout->addLayout(row, 1);
}

void SoftKeyboard::buildUi()
{
    int kbH = 380;
    if (QScreen *screen = QGuiApplication::primaryScreen()) {
        kbH = qBound(340, int(screen->size().height() * 0.40), 480);
    }
    setFixedHeight(kbH);

    auto *root = new QVBoxLayout(this);
    root->setContentsMargins(6, 6, 6, 8);
    root->setSpacing(6);

    m_composeLabel = new QLabel(this);
    m_composeLabel->setObjectName(QStringLiteral("imeCompose"));
    m_composeLabel->setFixedHeight(28);
    root->addWidget(m_composeLabel);

    // Candidate row: [上页] [横滑候选条] [下页] — page buttons are touch-sized.
    m_candidateHost = new QWidget(this);
    m_candidateHost->setObjectName(QStringLiteral("imeCandidateBar"));
    m_candidateHost->setFixedHeight(56);
    auto *candRoot = new QHBoxLayout(m_candidateHost);
    candRoot->setContentsMargins(0, 0, 0, 0);
    candRoot->setSpacing(4);

    m_prevPageBtn = new QPushButton(tr("上页"), m_candidateHost);
    m_prevPageBtn->setObjectName(QStringLiteral("imePageBtn"));
    m_prevPageBtn->setFocusPolicy(Qt::NoFocus);
    m_prevPageBtn->setMinimumWidth(72);
    m_prevPageBtn->setSizePolicy(QSizePolicy::Fixed, QSizePolicy::Expanding);
    connect(m_prevPageBtn, &QPushButton::clicked, this, [this] { pageCandidates(-1); });

    m_nextPageBtn = new QPushButton(tr("下页"), m_candidateHost);
    m_nextPageBtn->setObjectName(QStringLiteral("imePageBtn"));
    m_nextPageBtn->setFocusPolicy(Qt::NoFocus);
    m_nextPageBtn->setMinimumWidth(72);
    m_nextPageBtn->setSizePolicy(QSizePolicy::Fixed, QSizePolicy::Expanding);
    connect(m_nextPageBtn, &QPushButton::clicked, this, [this] { pageCandidates(1); });

    m_candidateScroll = new QScrollArea(m_candidateHost);
    m_candidateScroll->setObjectName(QStringLiteral("imeCandidateScroll"));
    m_candidateScroll->setWidgetResizable(false);
    m_candidateScroll->setFrameShape(QFrame::NoFrame);
    m_candidateScroll->setHorizontalScrollBarPolicy(Qt::ScrollBarAlwaysOff);
    m_candidateScroll->setVerticalScrollBarPolicy(Qt::ScrollBarAlwaysOff);
    m_candidateScroll->setFocusPolicy(Qt::NoFocus);
    m_candidateStrip = new QWidget(m_candidateScroll);
    m_candidateStrip->setObjectName(QStringLiteral("imeCandidateStrip"));
    m_candidateLayout = new QHBoxLayout(m_candidateStrip);
    m_candidateLayout->setContentsMargins(2, 2, 2, 2);
    m_candidateLayout->setSpacing(6);
    m_candidateScroll->setWidget(m_candidateStrip);
    touchscroll::enableOn(m_candidateScroll);

    candRoot->addWidget(m_prevPageBtn);
    candRoot->addWidget(m_candidateScroll, 1);
    candRoot->addWidget(m_nextPageBtn);
    root->addWidget(m_candidateHost);

    auto *keysHost = new QWidget(this);
    keysHost->setObjectName(QStringLiteral("imeKeysHost"));
    m_letterHostLayout = new QVBoxLayout(keysHost);
    m_letterHostLayout->setContentsMargins(0, 0, 0, 0);
    m_letterHostLayout->setSpacing(5);
    root->addWidget(keysHost, 1);

    rebuildKeys();
}

void SoftKeyboard::rebuildKeys()
{
    while (QLayoutItem *item = m_letterHostLayout->takeAt(0)) {
        if (QLayout *lay = item->layout()) {
            while (QLayoutItem *child = lay->takeAt(0)) {
                delete child->widget();
                delete child;
            }
        }
        delete item->widget();
        delete item;
    }
    m_modeBtn = nullptr;

    switch (m_page) {
    case Digits:
        rebuildDigitPage();
        break;
    case Symbols:
        rebuildSymbolPage();
        break;
    case Letters:
    default:
        rebuildLetterPage();
        break;
    }
}

void SoftKeyboard::rebuildLetterPage()
{
    const QString r1 = m_shift ? QStringLiteral("QWERTYUIOP") : QStringLiteral("qwertyuiop");
    const QString r2 = m_shift ? QStringLiteral("ASDFGHJKL") : QStringLiteral("asdfghjkl");
    const QString r3 = m_shift ? QStringLiteral("ZXCVBNM") : QStringLiteral("zxcvbnm");

    addCharRow(m_letterHostLayout, r1, 0);
    addCharRow(m_letterHostLayout, r2, 1);

    {
        auto *row = new QHBoxLayout;
        row->setSpacing(5);
        auto *shift = makeKey(QStringLiteral("⇧"), QStringLiteral("shift"), 15,
                              QStringLiteral("imeKeyWide"));
        connect(shift, &QPushButton::clicked, this, &SoftKeyboard::onSpecialClicked);
        row->addWidget(shift, 15);
        for (QChar ch : r3) {
            auto *btn = makeKey(QString(ch), QString(ch), 10);
            connect(btn, &QPushButton::clicked, this, &SoftKeyboard::onCharClicked);
            row->addWidget(btn, 10);
        }
        auto *bk = makeKey(QStringLiteral("⌫"), QStringLiteral("backspace"), 15,
                           QStringLiteral("imeKeyWide"));
        connect(bk, &QPushButton::clicked, this, &SoftKeyboard::onSpecialClicked);
        row->addWidget(bk, 15);
        m_letterHostLayout->addLayout(row, 1);
    }

    addBottomRow(false);
}

void SoftKeyboard::rebuildDigitPage()
{
    addCharRow(m_letterHostLayout, QStringLiteral("1234567890"));
    addCharRow(m_letterHostLayout, QStringLiteral("-/:;()$&@\""));
    {
        auto *row = new QHBoxLayout;
        row->setSpacing(5);
        auto *more = makeKey(QStringLiteral("#+="), QStringLiteral("page_symbols"), 15,
                             QStringLiteral("imeKeyWide"));
        connect(more, &QPushButton::clicked, this, &SoftKeyboard::onSpecialClicked);
        row->addWidget(more, 15);
        const QString mid = QStringLiteral(".,?!'\"");
        for (QChar ch : mid) {
            auto *btn = makeKey(QString(ch), QString(ch), 10);
            connect(btn, &QPushButton::clicked, this, &SoftKeyboard::onCharClicked);
            row->addWidget(btn, 10);
        }
        auto *bk = makeKey(QStringLiteral("⌫"), QStringLiteral("backspace"), 15,
                           QStringLiteral("imeKeyWide"));
        connect(bk, &QPushButton::clicked, this, &SoftKeyboard::onSpecialClicked);
        row->addWidget(bk, 15);
        m_letterHostLayout->addLayout(row, 1);
    }
    addBottomRow(false);
}

void SoftKeyboard::rebuildSymbolPage()
{
    addCharRow(m_letterHostLayout, QStringLiteral("[]{}#%^*+="));
    addCharRow(m_letterHostLayout, QStringLiteral("_\\|~<>€£¥•"));
    {
        auto *row = new QHBoxLayout;
        row->setSpacing(5);
        auto *more = makeKey(QStringLiteral("123"), QStringLiteral("page_digits"), 15,
                             QStringLiteral("imeKeyWide"));
        connect(more, &QPushButton::clicked, this, &SoftKeyboard::onSpecialClicked);
        row->addWidget(more, 15);
        const QString mid = QStringLiteral(".,?!'\"");
        for (QChar ch : mid) {
            auto *btn = makeKey(QString(ch), QString(ch), 10);
            connect(btn, &QPushButton::clicked, this, &SoftKeyboard::onCharClicked);
            row->addWidget(btn, 10);
        }
        auto *bk = makeKey(QStringLiteral("⌫"), QStringLiteral("backspace"), 15,
                           QStringLiteral("imeKeyWide"));
        connect(bk, &QPushButton::clicked, this, &SoftKeyboard::onSpecialClicked);
        row->addWidget(bk, 15);
        m_letterHostLayout->addLayout(row, 1);
    }
    addBottomRow(true);
}

void SoftKeyboard::addBottomRow(bool fromSymbolPage)
{
    Q_UNUSED(fromSymbolPage);
    auto *row = new QHBoxLayout;
    row->setSpacing(5);

    // 123 / ABC toggle
    const bool onLetters = (m_page == Letters);
    auto *pageBtn = makeKey(onLetters ? QStringLiteral("123") : QStringLiteral("ABC"),
                            onLetters ? QStringLiteral("page_digits") : QStringLiteral("page_letters"),
                            14, QStringLiteral("imeKeyWide"));
    connect(pageBtn, &QPushButton::clicked, this, &SoftKeyboard::onSpecialClicked);
    row->addWidget(pageBtn, 14);

    m_modeBtn = new QToolButton(this);
    m_modeBtn->setObjectName(QStringLiteral("imeKeyWide"));
    m_modeBtn->setSizePolicy(QSizePolicy::Expanding, QSizePolicy::Expanding);
    m_modeBtn->setText(m_mode == Pinyin ? tr("中/EN") : tr("EN/中"));
    m_modeBtn->setFocusPolicy(Qt::NoFocus);
    m_modeBtn->setProperty("code", QStringLiteral("mode"));
    connect(m_modeBtn, &QToolButton::clicked, this, &SoftKeyboard::onSpecialClicked);
    row->addWidget(m_modeBtn, 14);

    auto *hideBtn = makeKey(tr("收起"), QStringLiteral("hide"), 14, QStringLiteral("imeKeyWide"));
    connect(hideBtn, &QPushButton::clicked, this, &SoftKeyboard::onSpecialClicked);
    row->addWidget(hideBtn, 14);

    auto *space = makeKey(tr("空格"), QStringLiteral("space"), 36, QStringLiteral("imeKeySpace"));
    connect(space, &QPushButton::clicked, this, &SoftKeyboard::onSpecialClicked);
    row->addWidget(space, 36);

    auto *enter = makeKey(tr("换行"), QStringLiteral("enter"), 16, QStringLiteral("imeKeyWide"));
    connect(enter, &QPushButton::clicked, this, &SoftKeyboard::onSpecialClicked);
    row->addWidget(enter, 16);

    m_letterHostLayout->addLayout(row, 1);
}

void SoftKeyboard::onCharClicked()
{
    const auto *btn = qobject_cast<QPushButton *>(sender());
    if (!btn) {
        return;
    }
    const QString ch = btn->property("code").toString();
    // Digits / punctuation always commit as-is (even in Pinyin mode).
    if (m_page != Letters) {
        emit textCommitted(ch);
        return;
    }
    if (m_mode == Pinyin) {
        appendLatin(ch.toLower());
        return;
    }
    emit textCommitted(ch);
    if (m_shift) {
        m_shift = false;
        rebuildKeys();
    }
}

void SoftKeyboard::onSpecialClicked()
{
    QString code;
    if (auto *b = qobject_cast<QPushButton *>(sender())) {
        code = b->property("code").toString();
    } else if (auto *t = qobject_cast<QToolButton *>(sender())) {
        code = t->property("code").toString();
    }
    if (code == QLatin1String("shift")) {
        m_shift = !m_shift;
        rebuildKeys();
        return;
    }
    if (code == QLatin1String("page_digits")) {
        m_page = Digits;
        m_shift = false;
        rebuildKeys();
        return;
    }
    if (code == QLatin1String("page_symbols")) {
        m_page = Symbols;
        rebuildKeys();
        return;
    }
    if (code == QLatin1String("page_letters")) {
        m_page = Letters;
        rebuildKeys();
        return;
    }
    if (code == QLatin1String("mode")) {
        m_mode = (m_mode == English) ? Pinyin : English;
        clearComposing();
        m_page = Letters;
        rebuildKeys();
        return;
    }
    if (code == QLatin1String("hide")) {
        emit hideRequested();
        return;
    }
    if (code == QLatin1String("backspace")) {
        if (m_mode == Pinyin && !m_composing.isEmpty()) {
            m_composing.chop(1);
            m_composeLabel->setText(m_composing);
            updateCandidates();
            return;
        }
        emit backspacePressed();
        return;
    }
    if (code == QLatin1String("enter")) {
        if (m_mode == Pinyin && !m_composing.isEmpty()) {
            emit textCommitted(m_composing);
            clearComposing();
            return;
        }
        emit enterPressed();
        return;
    }
    if (code == QLatin1String("space")) {
        if (m_mode == Pinyin && !m_composing.isEmpty()) {
            if (!m_allCandidates.isEmpty()) {
                if (m_useEngine && !m_allCandIds.isEmpty()) {
                    commitEngineChoice(m_allCandIds.first());
                } else {
                    const QString py = m_allMatchedPy.value(0, m_composing);
                    commitPinyinChoice(m_allCandidates.first(), py);
                }
            } else {
                emit textCommitted(m_composing);
                clearComposing();
            }
            return;
        }
        emit textCommitted(QStringLiteral(" "));
    }
}

void SoftKeyboard::appendLatin(const QString &ch)
{
    if (ch.size() != 1 || !ch.at(0).isLetter()) {
        return;
    }
    if (m_composing.size() >= kMaxComposing) {
        return;
    }
    m_composing += ch;
    m_composeLabel->setText(m_composing);
    updateCandidates();
}

QStringList SoftKeyboard::lookupPinyin(const QString &py) const
{
    return m_dict.value(py.toLower());
}

QString SoftKeyboard::longestDictPrefix(const QString &s) const
{
    const int n = qMin(s.size(), m_maxKeyLen);
    for (int len = n; len >= 1; --len) {
        const QString pre = s.left(len);
        if (m_dict.contains(pre)) {
            return pre;
        }
    }
    return {};
}

void SoftKeyboard::addCandidate(const QString &word, const QString &matchedPy)
{
    if (word.isEmpty() || matchedPy.isEmpty()) {
        return;
    }
    for (int i = 0; i < m_allCandidates.size(); ++i) {
        if (m_allCandidates.at(i) == word && m_allMatchedPy.value(i) == matchedPy) {
            return;
        }
    }
    m_allCandidates.append(word);
    m_allMatchedPy.append(matchedPy);
}

void SoftKeyboard::prependCandidate(const QString &word, const QString &matchedPy)
{
    if (word.isEmpty() || matchedPy.isEmpty()) {
        return;
    }
    for (int i = 0; i < m_allCandidates.size(); ++i) {
        if (m_allCandidates.at(i) == word && m_allMatchedPy.value(i) == matchedPy) {
            return;
        }
    }
    m_allCandidates.prepend(word);
    m_allMatchedPy.prepend(matchedPy);
}

void SoftKeyboard::addAutoSegmentSentence(const QString &py)
{
    if (py.size() < 2) {
        return;
    }
    QString hanzi;
    QString consumed;
    QString rem = py;
    while (!rem.isEmpty()) {
        const QString pre = longestDictPrefix(rem);
        if (pre.isEmpty()) {
            break;
        }
        const QStringList words = lookupPinyin(pre);
        if (words.isEmpty()) {
            break;
        }
        hanzi += words.first();
        consumed += pre;
        rem = rem.mid(pre.size());
    }
    // Need at least two syllables / a phrase worth of coverage to show as 整句.
    if (hanzi.isEmpty() || consumed.size() < 2) {
        return;
    }
    // Prefer full coverage; still show partial long matches (e.g. leftover unknown tail).
    if (consumed.size() >= 4 || rem.isEmpty()) {
        prependCandidate(hanzi, consumed);
    }
}

void SoftKeyboard::updateCandidates()
{
    m_allCandidates.clear();
    m_allMatchedPy.clear();
    m_allCandIds.clear();
    m_candidatePage = 0;
    if (m_composing.isEmpty()) {
        if (m_useEngine) {
            GooglePinyinEngine::instance().resetSearch();
        }
        renderCandidatePage();
        return;
    }

    if (m_useEngine) {
        updateCandidatesFromEngine();
        renderCandidatePage();
        return;
    }

    const QString py = m_composing.toLower();
    addAutoSegmentSentence(py);
    for (const QString &w : lookupPinyin(py)) {
        addCandidate(w, py);
    }
    const QString prefix = longestDictPrefix(py);
    if (!prefix.isEmpty() && prefix != py) {
        for (const QString &w : lookupPinyin(prefix)) {
            addCandidate(w, prefix);
        }
    }
    renderCandidatePage();
}

void SoftKeyboard::updateCandidatesFromEngine()
{
    auto &eng = GooglePinyinEngine::instance();
    const int n = eng.search(m_composing.toLower());
    const int limit = qMin(n, 80);
    for (int i = 0; i < limit; ++i) {
        const QString w = eng.candidateAt(i);
        if (w.isEmpty()) {
            continue;
        }
        m_allCandidates.append(w);
        m_allMatchedPy.append(m_composing.toLower());
        m_allCandIds.append(i);
    }
}

void SoftKeyboard::commitEngineChoice(int candId)
{
    auto &eng = GooglePinyinEngine::instance();
    const QString text = eng.candidateAt(candId);
    if (!text.isEmpty()) {
        emit textCommitted(text);
    }
    eng.choose(candId);
    // Full-dict engine: after a choice, start fresh for the next phrase.
    // (Sentence-level candidate 0 already covers long pinyin in one tap.)
    clearComposing();
    eng.resetSearch();
}

void SoftKeyboard::renderCandidatePage()
{
    while (QLayoutItem *item = m_candidateLayout->takeAt(0)) {
        delete item->widget();
        delete item;
    }

    const int total = m_allCandidates.size();
    const int pages = total == 0 ? 1 : (total + kCandPerPage - 1) / kCandPerPage;
    if (m_candidatePage >= pages) {
        m_candidatePage = pages - 1;
    }
    if (m_candidatePage < 0) {
        m_candidatePage = 0;
    }

    const bool multi = total > kCandPerPage;
    m_prevPageBtn->setVisible(multi);
    m_nextPageBtn->setVisible(multi);
    m_prevPageBtn->setEnabled(multi && m_candidatePage > 0);
    m_nextPageBtn->setEnabled(multi && m_candidatePage < pages - 1);

    if (total == 0) {
        m_candidateStrip->setFixedSize(0, 0);
        return;
    }

    const int start = m_candidatePage * kCandPerPage;
    int stripW = 4;
    const int btnH = qMax(44, m_candidateHost->height() - 8);
    for (int i = start; i < start + kCandPerPage && i < total; ++i) {
        auto *btn = new QPushButton(m_allCandidates.at(i), m_candidateStrip);
        btn->setObjectName(QStringLiteral("imeCandidate"));
        btn->setFocusPolicy(Qt::NoFocus);
        btn->setMinimumWidth(kCandBtnMinWidth);
        btn->setFixedHeight(btnH);
        const int textW = btn->fontMetrics().horizontalAdvance(m_allCandidates.at(i)) + 28;
        btn->setFixedWidth(qMax(kCandBtnMinWidth, textW));
        btn->setProperty("word", m_allCandidates.at(i));
        btn->setProperty("matchedPy", m_allMatchedPy.value(i));
        btn->setProperty("candId", m_allCandIds.value(i, -1));
        connect(btn, &QPushButton::clicked, this, &SoftKeyboard::onCandidateClicked);
        m_candidateLayout->addWidget(btn);
        stripW += btn->width() + m_candidateLayout->spacing();
    }
    m_candidateStrip->setFixedSize(stripW, btnH + 4);
    m_candidateScroll->horizontalScrollBar()->setValue(0);
}

void SoftKeyboard::pageCandidates(int delta)
{
    if (m_allCandidates.isEmpty()) {
        return;
    }
    const int pages = (m_allCandidates.size() + kCandPerPage - 1) / kCandPerPage;
    m_candidatePage = qBound(0, m_candidatePage + delta, pages - 1);
    renderCandidatePage();
}

void SoftKeyboard::onCandidateClicked()
{
    const auto *btn = qobject_cast<QPushButton *>(sender());
    if (!btn) {
        return;
    }
    if (m_useEngine) {
        const int id = btn->property("candId").toInt();
        if (id >= 0) {
            commitEngineChoice(id);
            return;
        }
    }
    commitPinyinChoice(btn->property("word").toString(),
                       btn->property("matchedPy").toString());
}

void SoftKeyboard::commitPinyinChoice(const QString &hanzi, const QString &matchedPy)
{
    if (!hanzi.isEmpty()) {
        emit textCommitted(hanzi);
    }
    QString consume = matchedPy.toLower();
    if (consume.isEmpty()) {
        clearComposing();
        return;
    }
    // Prediction of a longer key (niha → 你好): consume only what was typed.
    if (consume.startsWith(m_composing.toLower()) && consume.size() > m_composing.size()) {
        clearComposing();
        return;
    }
    if (m_composing.toLower().startsWith(consume)) {
        m_composing = m_composing.mid(consume.size());
    } else {
        m_composing.clear();
    }
    m_composeLabel->setText(m_composing);
    updateCandidates();
}
