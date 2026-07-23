#include "googlepinyinengine.h"

#include <QCoreApplication>
#include <QDir>
#include <QFileInfo>
#include <QStandardPaths>

#include "pinyinime.h"

using ime_pinyin::char16;
using ime_pinyin::im_choose;
using ime_pinyin::im_close_decoder;
using ime_pinyin::im_get_candidate;
using ime_pinyin::im_open_decoder;
using ime_pinyin::im_reset_search;
using ime_pinyin::im_search;
using ime_pinyin::im_set_max_lens;

namespace {

QString char16ToQString(const char16 *s)
{
    if (!s) {
        return {};
    }
    // Engine stores UTF-16 code units; Qt expects char16_t / ushort.
    return QString::fromUtf16(reinterpret_cast<const char16_t *>(s));
}

QString resolveDictPath(const QString &preferred)
{
    const QStringList candidates = {
        preferred,
        QCoreApplication::applicationDirPath() + QStringLiteral("/ime/dict_pinyin.dat"),
        QCoreApplication::applicationDirPath() + QStringLiteral("/dict_pinyin.dat"),
        QStringLiteral("/root/mooncoding/ime/dict_pinyin.dat"),
        QDir::currentPath() + QStringLiteral("/ime/dict_pinyin.dat"),
    };
    for (const QString &p : candidates) {
        if (!p.isEmpty() && QFileInfo::exists(p) && QFileInfo(p).size() > 1000) {
            return p;
        }
    }
    return {};
}

} // namespace

GooglePinyinEngine &GooglePinyinEngine::instance()
{
    static GooglePinyinEngine eng;
    return eng;
}

GooglePinyinEngine::~GooglePinyinEngine()
{
    close();
}

bool GooglePinyinEngine::open(const QString &sysDictPath)
{
    close();
    const QString sys = resolveDictPath(sysDictPath);
    if (sys.isEmpty()) {
        return false;
    }

    const QString cfg = QStandardPaths::writableLocation(QStandardPaths::AppDataLocation);
    QDir().mkpath(cfg);
    const QString user = cfg + QStringLiteral("/dict_pinyin_user.dat");

    // Long pinyin sentences + long Chinese output.
    im_set_max_lens(64, 32);
    m_ready = im_open_decoder(sys.toLocal8Bit().constData(), user.toLocal8Bit().constData());
    m_count = 0;
    return m_ready;
}

void GooglePinyinEngine::close()
{
    if (m_ready) {
        im_close_decoder();
        m_ready = false;
    }
    m_count = 0;
}

void GooglePinyinEngine::resetSearch()
{
    if (!m_ready) {
        return;
    }
    im_reset_search();
    m_count = 0;
}

int GooglePinyinEngine::search(const QString &pinyin)
{
    if (!m_ready) {
        return 0;
    }
    const QByteArray py = pinyin.toLatin1().toLower();
    im_reset_search();
    m_count = int(im_search(py.constData(), size_t(py.size())));
    return m_count;
}

QString GooglePinyinEngine::candidateAt(int index) const
{
    if (!m_ready || index < 0 || index >= m_count) {
        return {};
    }
    char16 buf[256];
    buf[0] = 0;
    if (!im_get_candidate(size_t(index), buf, 255)) {
        return {};
    }
    buf[255] = 0;
    return char16ToQString(buf);
}

int GooglePinyinEngine::choose(int index)
{
    if (!m_ready || index < 0 || index >= m_count) {
        return 0;
    }
    m_count = int(im_choose(size_t(index)));
    return m_count;
}
