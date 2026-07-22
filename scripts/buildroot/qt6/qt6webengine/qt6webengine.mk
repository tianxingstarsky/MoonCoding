################################################################################
#
# qt6webengine
#
# Custom MoonCoding package (Buildroot upstream has Qt5 WebEngine only).
# Matches QT6_VERSION from package/qt6/qt6.mk (6.4.3 on Luckfox Lyra SDK).
#
################################################################################

QT6WEBENGINE_VERSION = $(QT6_VERSION)
QT6WEBENGINE_SITE = $(QT6_SITE)
QT6WEBENGINE_SOURCE = qtwebengine-$(QT6_SOURCE_TARBALL_PREFIX)-$(QT6WEBENGINE_VERSION).tar.xz
QT6WEBENGINE_INSTALL_STAGING = YES
QT6WEBENGINE_SUPPORTS_IN_SOURCE_BUILD = NO

QT6WEBENGINE_CMAKE_BACKEND = ninja

QT6WEBENGINE_LICENSE = GPL-2.0 or LGPL-3.0 or GPL-3.0 or GPL-3.0 with exception
QT6WEBENGINE_LICENSE_FILES = \
	LICENSES/GPL-2.0-only.txt \
	LICENSES/GPL-3.0-only.txt \
	LICENSES/LGPL-3.0-only.txt \
	LICENSES/Qt-GPL-exception-1.0.txt

# Keep the build smaller: skip PDF module, examples, tests.
QT6WEBENGINE_CONF_OPTS = \
	-DQT_HOST_PATH=$(HOST_DIR) \
	-DBUILD_WITH_PCH=OFF \
	-DQT_BUILD_EXAMPLES=OFF \
	-DQT_BUILD_TESTS=OFF \
	-DQT_FEATURE_qtpdf_build=OFF \
	-DQT_FEATURE_webengine_system_ninja=ON \
	-DQT_FEATURE_webengine_proprietary_codecs=OFF \
	-DQT_FEATURE_webengine_webrtc=OFF \
	-DQT_FEATURE_webengine_kerberos=OFF \
	-DQT_FEATURE_webengine_printing_and_pdf=OFF \
	-DQT_FEATURE_webengine_pepper_plugins=OFF \
	-DQT_FEATURE_webengine_spellchecker=OFF

QT6WEBENGINE_DEPENDENCIES = \
	host-bison \
	host-flex \
	host-gperf \
	host-ninja \
	host-nodejs \
	host-pkgconf \
	host-python3 \
	fontconfig \
	freetype \
	jpeg \
	libnss \
	libpng \
	libxml2 \
	libxslt \
	qt6base \
	qt6webchannel \
	zlib

# dbus is optional but helps Chromium sandboxes / notifications
ifeq ($(BR2_PACKAGE_DBUS),y)
QT6WEBENGINE_DEPENDENCIES += dbus
endif

$(eval $(cmake-package))
