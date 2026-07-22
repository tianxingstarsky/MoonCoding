################################################################################
#
# qt6webchannel
#
################################################################################

QT6WEBCHANNEL_VERSION = $(QT6_VERSION)
QT6WEBCHANNEL_SITE = $(QT6_SITE)
QT6WEBCHANNEL_SOURCE = qtwebchannel-$(QT6_SOURCE_TARBALL_PREFIX)-$(QT6WEBCHANNEL_VERSION).tar.xz
QT6WEBCHANNEL_INSTALL_STAGING = YES
QT6WEBCHANNEL_SUPPORTS_IN_SOURCE_BUILD = NO

QT6WEBCHANNEL_CMAKE_BACKEND = ninja

QT6WEBCHANNEL_LICENSE = \
	GPL-2.0+ or LGPL-3.0, \
	GPL-3.0 with exception (tools), \
	GFDL-1.3 (docs), \
	BSD-3-Clause

QT6WEBCHANNEL_LICENSE_FILES = \
	LICENSES/BSD-3-Clause.txt \
	LICENSES/GFDL-1.3-no-invariants-only.txt \
	LICENSES/GPL-2.0-only.txt \
	LICENSES/GPL-3.0-only.txt \
	LICENSES/LGPL-3.0-only.txt \
	LICENSES/Qt-GPL-exception-1.0.txt

QT6WEBCHANNEL_CONF_OPTS = \
	-DQT_HOST_PATH=$(HOST_DIR) \
	-DBUILD_WITH_PCH=OFF \
	-DQT_BUILD_EXAMPLES=OFF \
	-DQT_BUILD_TESTS=OFF

QT6WEBCHANNEL_DEPENDENCIES = \
	host-pkgconf \
	qt6base

$(eval $(cmake-package))
